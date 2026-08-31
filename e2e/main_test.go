package main

import (
	"bytes"
	"context"
	"image"
	"image/color"
	"image/png"
	"os"
	"reflect"
	"sort"
	"strings"
	"testing"
)

func TestRegistryCoversEverySupportedLanguageAndValidation(t *testing.T) {
	cases := registry()
	if err := validateRegistry(cases); err != nil {
		t.Fatal(err)
	}
	languages := map[string]bool{}
	validation := false
	for _, c := range cases {
		languages[c.Language] = true
		validation = validation || strings.HasPrefix(c.ID, "VAL-")
		if _, err := fixtureFS.ReadFile(c.Fixture); err != nil {
			t.Fatalf("%s fixture: %v", c.ID, err)
		}
	}
	want := []string{"d2", "dbml", "likec4", "nomnoml", "pikchr", "structurizr", "wavedrom"}
	got := make([]string, 0, len(languages))
	for language := range languages {
		got = append(got, language)
	}
	sort.Strings(got)
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("languages = %v, want %v", got, want)
	}
	if !validation {
		t.Fatal("registry has no validation case")
	}
}

func TestValidateRegistryRejectsIncompleteAndDuplicateCases(t *testing.T) {
	duplicate := []caseDef{registry()[0], registry()[0]}
	if err := validateRegistry(duplicate); err == nil || !strings.Contains(err.Error(), "duplicate case id PAR-001") {
		t.Fatalf("duplicate ids should be rejected, got %v", err)
	}
	incomplete := []caseDef{{ID: "PAR-999", Description: "missing execution metadata", Tags: []string{"parity"}}}
	if err := validateRegistry(incomplete); err == nil || !strings.Contains(err.Error(), "invalid case registration") {
		t.Fatalf("incomplete registration should be rejected, got %v", err)
	}
}

func TestFeatureMatrixIsCompleteAndBidirectionallyBound(t *testing.T) {
	matrix, err := loadFeatureMatrix()
	if err != nil {
		t.Fatal(err)
	}
	if err := validateFeatureCoverage(registry(), matrix); err != nil {
		t.Fatal(err)
	}
	aligned, excluded := 0, 0
	for _, feature := range matrix.Features {
		switch feature.Status {
		case "aligned":
			aligned++
		case "intentional-exclusion":
			excluded++
		}
	}
	if aligned < 50 || excluded < 20 {
		t.Fatalf("feature inventory unexpectedly shrank: aligned=%d excluded=%d", aligned, excluded)
	}
}

func TestFeatureMatrixGuardRejectsUnboundFeature(t *testing.T) {
	matrix, err := loadFeatureMatrix()
	if err != nil {
		t.Fatal(err)
	}
	for index := range matrix.Features {
		if matrix.Features[index].Status == "aligned" {
			matrix.Features[index].Cases = []string{"PAR-999"}
			break
		}
	}
	if err := validateFeatureCoverage(registry(), matrix); err == nil || !strings.Contains(err.Error(), "unknown case PAR-999") {
		t.Fatalf("unbound feature should fail, got %v", err)
	}
}

func TestSelectCasesUsesRegistryAndRejectsUnknownIDs(t *testing.T) {
	selected, err := selectCases(registry(), []string{"par-003,VAL-001"}, "")
	if err != nil {
		t.Fatal(err)
	}
	if got := []string{selected[0].ID, selected[1].ID}; !reflect.DeepEqual(got, []string{"PAR-003", "VAL-001"}) {
		t.Fatalf("selection = %v", got)
	}
	if _, err := selectCases(registry(), []string{"PAR-999"}, ""); err == nil || err.Error() != "unknown case selection: PAR-999" {
		t.Fatalf("unknown selector error = %v", err)
	}
	if _, err := selectCases(registry(), nil, "nonexistent"); err == nil || err.Error() != "selection matched no cases" {
		t.Fatalf("empty tag selection error = %v", err)
	}
}

func TestDeterministicEnvDoesNotLeakCredentialsOrProxy(t *testing.T) {
	t.Setenv("HTTP_PROXY", "http://should-not-leak")
	t.Setenv("VENDOR_API_KEY", "should-not-leak")
	t.Setenv("GRAPHVIZ_DOT", "/opt/graphviz/bin/dot")
	home := t.TempDir()
	env := deterministicEnv(home)
	joined := strings.Join(env, "\n")
	if strings.Contains(joined, "should-not-leak") || strings.Contains(joined, "HTTP_PROXY") || strings.Contains(joined, "API_KEY") {
		t.Fatalf("sensitive ambient environment leaked:\n%s", joined)
	}
	if !strings.Contains(joined, "TZ=UTC") || !strings.Contains(joined, "HOME="+home) {
		t.Fatalf("deterministic controls missing:\n%s", joined)
	}
	if !strings.Contains(joined, "GRAPHVIZ_DOT=/opt/graphviz/bin/dot") {
		t.Fatalf("allowed passthrough key dropped:\n%s", joined)
	}
	if !sort.StringsAreSorted(env) {
		t.Fatalf("environment must stay sorted: %v", env)
	}
}

func TestVisualComparatorSeparatesSameAndDifferentLayouts(t *testing.T) {
	one := whiteImage(80, 50)
	two := whiteImage(80, 50)
	for y := 5; y < 45; y++ {
		for x := 8; x < 25; x++ {
			one.Set(x, y, color.Black)
			two.Set(x, y, color.Black)
		}
	}
	directory := t.TempDir()
	onePath, twoPath := directory+"/one.png", directory+"/two.png"
	writeTestPNG(t, onePath, one)
	writeTestPNG(t, twoPath, two)
	oneFact, err := readImageFact(onePath)
	if err != nil {
		t.Fatal(err)
	}
	twoFact, err := readImageFact(twoPath)
	if err != nil {
		t.Fatal(err)
	}
	same := compareImages(oneFact, twoFact)
	if same.MaskIoU != 1 || same.AspectDrift != 0 || same.InkCoverageDrift != 0 {
		t.Fatalf("identical layouts should match exactly: %+v", same)
	}
	for y := 0; y < 50; y++ {
		for x := 0; x < 80; x++ {
			two.Set(x, y, color.White)
		}
	}
	for y := 20; y < 32; y++ {
		for x := 5; x < 75; x++ {
			two.Set(x, y, color.Black)
		}
	}
	writeTestPNG(t, twoPath, two)
	twoFact, err = readImageFact(twoPath)
	if err != nil {
		t.Fatal(err)
	}
	different := compareImages(oneFact, twoFact)
	if different.AspectDrift < 0.8 {
		t.Fatalf("different layouts should be separated: %+v", different)
	}
}

func TestSVGTextExtractionIncludesNestedTspans(t *testing.T) {
	path := t.TempDir() + "/nested.svg"
	data := `<svg xmlns="http://www.w3.org/2000/svg"><text>Hello <tspan>Payments</tspan></text><path d="M0 0"/></svg>`
	if err := os.WriteFile(path, []byte(data), 0o644); err != nil {
		t.Fatal(err)
	}
	text, err := readSVGText(path)
	if err != nil {
		t.Fatal(err)
	}
	if !containsNormalized(text, "hello payments") {
		t.Fatalf("extracted text = %q", text)
	}
}

func TestCasePanicBecomesFailedResult(t *testing.T) {
	c := caseDef{
		ID: "VAL-999", Description: "panic probe", Tags: []string{"validation"}, Modality: "cli", Coverage: "validation", Fixture: "fixtures/invalid.dbml", Language: "dbml",
		Run: func(context.Context, *caseEnv, caseDef) caseResult { panic("boom") },
	}
	result := executeCase(context.Background(), &caseEnv{}, c)
	if result.Status != "failed" || !strings.Contains(result.Error, "case-panic") || !strings.Contains(result.Output, "executeCase") {
		t.Fatalf("panic did not become actionable failure: %+v", result)
	}
}

func TestRunDispatchesPureCommands(t *testing.T) {
	usage := "usage: diagram-render-e2e [--diagram-render PATH] [--original PATH] <doctor|list|matrix|run|version>\n"
	tests := []struct {
		args       []string
		wantExit   int
		wantStdout string
		wantStderr string
	}{
		{[]string{"version"}, exitOK, version + "\n", ""},
		{[]string{"--help"}, exitOK, usage, ""},
		{nil, exitUsage, "", usage},
		{[]string{"unknown"}, exitUsage, "", "unknown command \"unknown\"\n"},
	}
	for _, test := range tests {
		var stdout, stderr bytes.Buffer
		if got := run(test.args, &stdout, &stderr); got != test.wantExit {
			t.Fatalf("run(%v) = %d, want %d", test.args, got, test.wantExit)
		}
		if stdout.String() != test.wantStdout || stderr.String() != test.wantStderr {
			t.Fatalf("run(%v) stdout=%q stderr=%q", test.args, stdout.String(), stderr.String())
		}
	}
}

func TestParseGlobalRequiresValuesAndPreservesCommandFlags(t *testing.T) {
	if _, _, err := parseGlobal([]string{"--diagram-render"}); err == nil {
		t.Fatal("missing --diagram-render value should fail")
	}
	if _, _, err := parseGlobal([]string{"--original"}); err == nil {
		t.Fatal("missing --original value should fail")
	}
	global, rest, err := parseGlobal([]string{"--json", "--original", "/bin/true", "run", "--select", "PAR-001"})
	if err != nil {
		t.Fatal(err)
	}
	if !global.json || global.original != "/bin/true" || !reflect.DeepEqual(rest, []string{"run", "--select", "PAR-001"}) {
		t.Fatalf("global=%+v rest=%v", global, rest)
	}
}

func TestRelativeDriftZeroExpectationSemantics(t *testing.T) {
	if got := relativeDrift(110, 100); mathAbs(got-0.1) > 1e-12 {
		t.Fatalf("relative drift = %v", got)
	}
	if relativeDrift(0, 0) != 0 || relativeDrift(1, 0) != 1 {
		t.Fatal("zero expectation semantics changed")
	}
}

func whiteImage(width, height int) *image.RGBA {
	img := image.NewRGBA(image.Rect(0, 0, width, height))
	for y := 0; y < height; y++ {
		for x := 0; x < width; x++ {
			img.Set(x, y, color.White)
		}
	}
	return img
}

func writeTestPNG(t *testing.T, path string, img image.Image) {
	t.Helper()
	file, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	if err := png.Encode(file, img); err != nil {
		t.Fatal(err)
	}
}

func mathAbs(value float64) float64 {
	if value < 0 {
		return -value
	}
	return value
}
