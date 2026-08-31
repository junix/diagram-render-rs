// Command diagram-render-e2e compares diagram-render-rs with the original
// plot-provider-diagrams CLI through both tools' public rendering contracts.
package main

import (
	"bytes"
	"context"
	"embed"
	"encoding/json"
	"encoding/xml"
	"errors"
	"flag"
	"fmt"
	"image"
	"image/png"
	"io"
	"math"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"runtime/debug"
	"sort"
	"strings"
	"time"
)

const (
	version         = "0.1.0"
	exitOK          = 0
	exitFailed      = 1
	exitUsage       = 2
	exitUnavailable = 3
	gridSize        = 64
)

//go:embed fixtures/* feature_matrix.json
var fixtureFS embed.FS

type globalOptions struct {
	diagramRender string
	original      string
	json          bool
}

type target struct {
	Name         string   `json:"name"`
	Kind         string   `json:"kind"`
	Path         string   `json:"path,omitempty"`
	Available    bool     `json:"available"`
	Required     bool     `json:"required"`
	Capabilities []string `json:"capabilities,omitempty"`
	Missing      []string `json:"missing,omitempty"`
	Hint         string   `json:"hint,omitempty"`
}

type thresholds struct {
	MaxAspectDrift float64 `json:"max_aspect_drift"`
	MaxInkDrift    float64 `json:"max_ink_drift"`
	MinMaskIoU     float64 `json:"min_mask_iou"`
}

type caseDef struct {
	ID              string                                              `json:"id"`
	Description     string                                              `json:"description"`
	Tags            []string                                            `json:"tags"`
	Modality        string                                              `json:"modality"`
	Coverage        string                                              `json:"coverage"`
	Requires        []string                                            `json:"requires"`
	Fixture         string                                              `json:"-"`
	Language        string                                              `json:"language"`
	OriginalView    string                                              `json:"original_view,omitempty"`
	OriginalPNGOnly bool                                                `json:"original_png_only,omitempty"`
	ExpectedLabels  []string                                            `json:"expected_labels,omitempty"`
	CandidateLabels []string                                            `json:"candidate_labels,omitempty"`
	OriginalLabels  []string                                            `json:"original_labels,omitempty"`
	Features        []string                                            `json:"features,omitempty"`
	Thresholds      thresholds                                          `json:"thresholds"`
	Run             func(context.Context, *caseEnv, caseDef) caseResult `json:"-"`
}

type featureMatrix struct {
	SchemaVersion int          `json:"schema_version"`
	Scope         string       `json:"scope"`
	Oracle        string       `json:"oracle"`
	Features      []featureDef `json:"features"`
}

type featureDef struct {
	ID        string   `json:"id"`
	Language  string   `json:"language"`
	Status    string   `json:"status"`
	Cases     []string `json:"cases"`
	Assertion string   `json:"assertion,omitempty"`
	Reason    string   `json:"reason,omitempty"`
}

type caseResult struct {
	ID          string   `json:"id"`
	Description string   `json:"description"`
	Status      string   `json:"status"`
	DurationMS  int64    `json:"duration_ms"`
	Error       string   `json:"error,omitempty"`
	Output      string   `json:"output,omitempty"`
	SkipReason  string   `json:"skip_reason,omitempty"`
	Artifacts   []string `json:"artifacts"`
}

type report struct {
	SchemaVersion int          `json:"schema_version"`
	Targets       []target     `json:"targets"`
	Total         int          `json:"total"`
	Passed        int          `json:"passed"`
	Failed        int          `json:"failed"`
	Skipped       int          `json:"skipped"`
	DurationMS    int64        `json:"duration_ms"`
	Cases         []caseResult `json:"cases"`
}

type caseEnv struct {
	targets   []target
	workspace string
	keep      bool
}

type execResult struct {
	Argv       []string
	ExitCode   int
	DurationMS int64
	Stdout     string
	Stderr     string
}

type imageFact struct {
	Width       int       `json:"width"`
	Height      int       `json:"height"`
	InkWidth    int       `json:"ink_width"`
	InkHeight   int       `json:"ink_height"`
	AspectRatio float64   `json:"ink_aspect_ratio"`
	InkCoverage float64   `json:"normalized_ink_coverage"`
	ToneBuckets int       `json:"tone_buckets"`
	Mask        []float64 `json:"-"`
}

type parityFact struct {
	DiagramRender    imageFact `json:"diagram_render_rs"`
	Original         imageFact `json:"plot_provider_diagrams"`
	AspectDrift      float64   `json:"ink_aspect_drift"`
	InkCoverageDrift float64   `json:"ink_coverage_drift"`
	MaskIoU          float64   `json:"normalized_mask_iou"`
}

type originalDoctor struct {
	Items []struct {
		Backend   string   `json:"backend"`
		Available bool     `json:"available"`
		Missing   []string `json:"missing"`
	} `json:"items"`
}

func main() { os.Exit(run(os.Args[1:], os.Stdout, os.Stderr)) }

func run(args []string, stdout, stderr io.Writer) int {
	global, rest, err := parseGlobal(args)
	if err != nil {
		fmt.Fprintln(stderr, err)
		return exitUsage
	}
	if len(rest) == 0 {
		printHelp(stderr)
		return exitUsage
	}
	switch rest[0] {
	case "version", "--version", "-V":
		fmt.Fprintln(stdout, version)
		return exitOK
	case "doctor":
		return runDoctor(rest[1:], global, stdout, stderr)
	case "list":
		return runList(rest[1:], global, stdout, stderr)
	case "matrix":
		return runMatrix(rest[1:], global, stdout, stderr)
	case "run":
		return runCases(rest[1:], global, stdout, stderr)
	case "help", "--help", "-h":
		printHelp(stdout)
		return exitOK
	default:
		fmt.Fprintf(stderr, "unknown command %q\n", rest[0])
		return exitUsage
	}
}

func parseGlobal(args []string) (globalOptions, []string, error) {
	var opts globalOptions
	for len(args) > 0 {
		switch args[0] {
		case "--diagram-render":
			if len(args) < 2 {
				return opts, nil, errors.New("--diagram-render requires a path")
			}
			opts.diagramRender, args = args[1], args[2:]
		case "--original":
			if len(args) < 2 {
				return opts, nil, errors.New("--original requires a path")
			}
			opts.original, args = args[1], args[2:]
		case "--json":
			opts.json, args = true, args[1:]
		default:
			return opts, args, nil
		}
	}
	return opts, nil, nil
}

func printHelp(w io.Writer) {
	fmt.Fprintln(w, "usage: diagram-render-e2e [--diagram-render PATH] [--original PATH] <doctor|list|matrix|run|version>")
}

func registry() []caseDef {
	common := thresholds{MaxAspectDrift: 0.85, MaxInkDrift: 0.80, MinMaskIoU: 0.10}
	sparseOracle := thresholds{MaxAspectDrift: 0.85, MaxInkDrift: 1.60, MinMaskIoU: 0.10}
	return []caseDef{
		{ID: "PAR-001", Description: "DBML core schema semantics and visual structure", Tags: []string{"parity", "oracle", "dbml", "smoke"}, Modality: "cli", Coverage: "feature", Requires: []string{"original"}, Fixture: "fixtures/dbml.dbml", Language: "dbml", ExpectedLabels: []string{"users", "orders", "user_status", "PK"}, CandidateLabels: []string{"commerce", "UNIQUE", "NOT NULL"}, Features: []string{"dbml.tables", "dbml.table-schema-aliases", "dbml.columns-types", "dbml.column-primary-keys", "dbml.enums-values", "dbml.references"}, Thresholds: common, Run: runParity},
		{ID: "PAR-002", Description: "WaveDrom timing lane and group structure", Tags: []string{"parity", "oracle", "wavedrom"}, Modality: "cli", Coverage: "feature", Requires: []string{"original"}, Fixture: "fixtures/wavedrom.json5", Language: "wavedrom", ExpectedLabels: []string{"Bus transaction", "clk", "data", "valid"}, Features: []string{"wavedrom.timing-lanes", "wavedrom.nested-groups", "wavedrom.header-text"}, Thresholds: common, Run: runParity},
		{ID: "PAR-003", Description: "D2 entries, maps, properties, and directed edges", Tags: []string{"parity", "oracle", "d2", "smoke"}, Modality: "cli", Coverage: "feature", Requires: []string{"original"}, Fixture: "fixtures/d2.d2", Language: "d2", ExpectedLabels: []string{"Client", "Payments API", "PostgreSQL"}, CandidateLabels: []string{"shape: cylinder", "SQL"}, Features: []string{"d2.scalar-entries", "d2.map-entries", "d2.map-labels", "d2.property-annotations", "d2.directed-edges", "d2.edge-labels"}, Thresholds: common, Run: runParity},
		{ID: "PAR-004", Description: "Structurizr workspace and context relationships", Tags: []string{"parity", "oracle", "structurizr", "view"}, Modality: "cli", Coverage: "feature", Requires: []string{"original"}, Fixture: "fixtures/structurizr.dsl", Language: "structurizr", OriginalView: "context", ExpectedLabels: []string{"Customer", "Payments"}, Features: []string{"structurizr.workspace-name", "structurizr.elements", "structurizr.relationships", "structurizr.model-blocks"}, Thresholds: common, Run: runParity},
		{ID: "PAR-005", Description: "LikeC4 model, extend, property, and relationship semantics", Tags: []string{"parity", "oracle", "likec4", "view", "slow"}, Modality: "cli", Coverage: "feature", Requires: []string{"original"}, Fixture: "fixtures/likec4.c4", Language: "likec4", OriginalView: "parity", OriginalPNGOnly: true, ExpectedLabels: []string{"Coffee API", "Payments", "Ledger", "Worker"}, CandidateLabels: []string{"technology: Rust", "inside api"}, Features: []string{"likec4.model-sections", "likec4.elements", "likec4.nesting", "likec4.explicit-relationships", "likec4.relationship-labels", "likec4.extend-blocks"}, Thresholds: common, Run: runParity},
		{ID: "PAR-006", Description: "nomnoml classifiers, compartments, and directed associations", Tags: []string{"parity", "oracle", "nomnoml"}, Modality: "cli", Coverage: "feature", Requires: []string{"original"}, Fixture: "fixtures/nomnoml.nomnoml", Language: "nomnoml", ExpectedLabels: []string{"User", "API", "PostgreSQL"}, Features: []string{"nomnoml.classifier-types", "nomnoml.classifier-attributes", "nomnoml.compartments", "nomnoml.directed-associations", "nomnoml.dashed-associations"}, Thresholds: common, Run: runParity},
		{ID: "PAR-007", Description: "Pikchr representative labeled object flow", Tags: []string{"parity", "oracle", "pikchr"}, Modality: "cli", Coverage: "contract", Requires: []string{"original"}, Fixture: "fixtures/pikchr.pikchr", Language: "pikchr", ExpectedLabels: []string{"Client", "API", "PostgreSQL"}, Thresholds: common, Run: runParity},
		{ID: "PAR-008", Description: "DBML extended declarations and all reference cardinalities", Tags: []string{"parity", "oracle", "dbml"}, Modality: "cli", Coverage: "feature", Requires: []string{"original"}, Fixture: "fixtures/dbml_extended.dbml", Language: "dbml", ExpectedLabels: []string{"accounts", "memberships", "account_status"}, CandidateLabels: []string{"1 index(es)", "1 checks", "Primary account table", "uses ~audit_columns", "TABLE PARTIAL", "TABLE GROUP", "N:1", "1:N", "1:1", "N:N"}, OriginalLabels: []string{"1", "*"}, Features: []string{"dbml.reference-cardinalities"}, Thresholds: common, Run: runParity},
		{ID: "PAR-009", Description: "WaveDrom symbol, data, node-edge, and footer matrix", Tags: []string{"parity", "oracle", "wavedrom"}, Modality: "cli", Coverage: "feature", Requires: []string{"original"}, Fixture: "fixtures/wavedrom_symbols.json5", Language: "wavedrom", ExpectedLabels: []string{"Wave symbol matrix", "levels", "bus", "D0", "End of symbols", "transfer"}, Features: []string{"wavedrom.logic-levels", "wavedrom.clocks", "wavedrom.continuations", "wavedrom.unknown-cells", "wavedrom.high-impedance", "wavedrom.bus-cells", "wavedrom.data-labels", "wavedrom.node-markers", "wavedrom.node-edges", "wavedrom.footer-text"}, Thresholds: common, Run: runParity},
		{ID: "PAR-010", Description: "WaveDrom register field widths and ranges", Tags: []string{"parity", "oracle", "wavedrom"}, Modality: "cli", Coverage: "feature", Requires: []string{"original"}, Fixture: "fixtures/wavedrom_register.json5", Language: "wavedrom", ExpectedLabels: []string{"opcode", "mode", "valid", "payload"}, Features: []string{"wavedrom.register-fields"}, Thresholds: common, Run: runParity},
		{ID: "PAR-011", Description: "D2 nested maps, valueless entries, operators, and chains", Tags: []string{"parity", "oracle", "d2"}, Modality: "cli", Coverage: "feature", Requires: []string{"original"}, Fixture: "fixtures/d2_operators.d2", Language: "d2", ExpectedLabels: []string{"Alpha", "Beta", "Gamma", "Delta", "Worker"}, Features: []string{"d2.valueless-entries", "d2.nested-maps", "d2.reverse-edges", "d2.undirected-edges", "d2.bidirectional-edges", "d2.edge-chains"}, Thresholds: sparseOracle, Run: runParity},
		{ID: "PAR-012", Description: "Structurizr container details, nesting, properties, and relationship metadata", Tags: []string{"parity", "oracle", "structurizr", "view"}, Modality: "cli", Coverage: "feature", Requires: []string{"original"}, Fixture: "fixtures/structurizr.dsl", Language: "structurizr", OriginalView: "containers", ExpectedLabels: []string{"API", "Database", "Handles payment requests", "Rust", "Reads from and writes to", "SQL"}, CandidateLabels: []string{"Core"}, Features: []string{"structurizr.element-details", "structurizr.nesting", "structurizr.relationship-details"}, Thresholds: common, Run: runParity},
		{ID: "PAR-013", Description: "nomnoml association direction, style, and labels", Tags: []string{"parity", "oracle", "nomnoml"}, Modality: "cli", Coverage: "feature", Requires: []string{"original"}, Fixture: "fixtures/nomnoml_associations.nomnoml", Language: "nomnoml", ExpectedLabels: []string{"Alpha", "Beta", "Gamma", "Delta", "one", "many"}, Features: []string{"nomnoml.reverse-associations", "nomnoml.undirected-associations", "nomnoml.bidirectional-associations", "nomnoml.relation-labels"}, Thresholds: common, Run: runParity},
		{ID: "PAR-014", Description: "Pikchr shape, direction, and flow-object surface", Tags: []string{"parity", "oracle", "pikchr"}, Modality: "cli", Coverage: "feature", Requires: []string{"original"}, Fixture: "fixtures/pikchr_surface.pikchr", Language: "pikchr", ExpectedLabels: []string{"Box", "Circle", "Ellipse", "Oval", "Diamond", "Cylinder", "Text", "Down", "Left", "Up"}, Features: []string{"pikchr.default-boxes", "pikchr.circles", "pikchr.ellipses-ovals", "pikchr.diamonds", "pikchr.cylinders", "pikchr.dots", "pikchr.text-objects", "pikchr.labels", "pikchr.directions", "pikchr.arrows", "pikchr.lines", "pikchr.splines-arcs", "pikchr.moves"}, Thresholds: common, Run: runParity},
		{ID: "VAL-001", Description: "both CLIs reject an unterminated DBML table", Tags: []string{"validation", "oracle", "dbml", "smoke"}, Modality: "cli", Coverage: "validation", Requires: []string{"original"}, Fixture: "fixtures/invalid.dbml", Language: "dbml", Run: runValidation},
	}
}

func validateRegistry(cases []caseDef) error {
	seen := map[string]bool{}
	for _, c := range cases {
		if c.ID == "" || c.Description == "" || len(c.Tags) == 0 || c.Modality == "" || c.Coverage == "" || c.Fixture == "" || c.Language == "" || c.Run == nil {
			return fmt.Errorf("invalid case registration: %+v", c)
		}
		if seen[c.ID] {
			return fmt.Errorf("duplicate case id %s", c.ID)
		}
		seen[c.ID] = true
	}
	matrix, err := loadFeatureMatrix()
	if err != nil {
		return err
	}
	return validateFeatureCoverage(cases, matrix)
}

func loadFeatureMatrix() (featureMatrix, error) {
	data, err := fixtureFS.ReadFile("feature_matrix.json")
	if err != nil {
		return featureMatrix{}, fmt.Errorf("read feature matrix: %w", err)
	}
	var matrix featureMatrix
	if err := json.Unmarshal(data, &matrix); err != nil {
		return featureMatrix{}, fmt.Errorf("decode feature matrix: %w", err)
	}
	return matrix, nil
}

func validateFeatureCoverage(cases []caseDef, matrix featureMatrix) error {
	if matrix.SchemaVersion != 1 || matrix.Scope == "" || matrix.Oracle == "" || len(matrix.Features) == 0 {
		return errors.New("feature matrix metadata is incomplete")
	}
	caseByID := make(map[string]caseDef, len(cases))
	for _, c := range cases {
		caseByID[c.ID] = c
	}
	featureByID := make(map[string]featureDef, len(matrix.Features))
	for _, feature := range matrix.Features {
		if feature.ID == "" || feature.Language == "" {
			return fmt.Errorf("invalid feature registration: %+v", feature)
		}
		if _, exists := featureByID[feature.ID]; exists {
			return fmt.Errorf("duplicate feature id %s", feature.ID)
		}
		featureByID[feature.ID] = feature
		switch feature.Status {
		case "aligned":
			if feature.Assertion == "" || len(feature.Cases) == 0 {
				return fmt.Errorf("aligned feature %s has no assertion or case", feature.ID)
			}
			for _, caseID := range feature.Cases {
				registered, ok := caseByID[caseID]
				if !ok {
					return fmt.Errorf("feature %s references unknown case %s", feature.ID, caseID)
				}
				if registered.Language != feature.Language || !contains(registered.Features, feature.ID) {
					return fmt.Errorf("feature %s and case %s are not bound bidirectionally", feature.ID, caseID)
				}
			}
		case "intentional-exclusion":
			if feature.Reason == "" || len(feature.Cases) != 0 {
				return fmt.Errorf("excluded feature %s must have a reason and no cases", feature.ID)
			}
		default:
			return fmt.Errorf("feature %s has unknown status %q", feature.ID, feature.Status)
		}
	}
	for _, c := range cases {
		if strings.HasPrefix(c.ID, "PAR-") && c.Coverage == "feature" && len(c.Features) == 0 {
			return fmt.Errorf("feature case %s has no feature bindings", c.ID)
		}
		seenFeatures := map[string]bool{}
		for _, featureID := range c.Features {
			if seenFeatures[featureID] {
				return fmt.Errorf("case %s repeats feature %s", c.ID, featureID)
			}
			seenFeatures[featureID] = true
			feature, ok := featureByID[featureID]
			if !ok || feature.Status != "aligned" || !contains(feature.Cases, c.ID) {
				return fmt.Errorf("case %s references unaligned or unbound feature %s", c.ID, featureID)
			}
		}
	}
	return nil
}

func runDoctor(args []string, global globalOptions, stdout, stderr io.Writer) int {
	fs := flag.NewFlagSet("doctor", flag.ContinueOnError)
	fs.SetOutput(stderr)
	jsonOutput := fs.Bool("json", global.json, "emit JSON")
	if err := fs.Parse(args); err != nil {
		return exitUsage
	}
	targets := resolveTargets(global)
	probeOriginal(&targets, languagesForCases(registry()))
	ok := targetsAvailable(targets)
	if *jsonOutput {
		missing := []string{}
		hints := []string{}
		for _, t := range targets {
			if !t.Available {
				missing = append(missing, t.Name)
			}
			if t.Hint != "" {
				hints = append(hints, t.Hint)
			}
		}
		_ = json.NewEncoder(stdout).Encode(map[string]any{"ok": ok, "targets": targets, "missing": missing, "hints": hints})
	} else {
		for _, t := range targets {
			state := "ok"
			if !t.Available {
				state = "missing"
			}
			fmt.Fprintf(stdout, "%-24s %-7s %s\n", t.Name, state, firstNonEmpty(t.Path, t.Hint))
			for _, missing := range t.Missing {
				fmt.Fprintf(stdout, "  missing: %s\n", missing)
			}
		}
	}
	if !ok {
		return exitUnavailable
	}
	return exitOK
}

func runList(args []string, global globalOptions, stdout, stderr io.Writer) int {
	fs := flag.NewFlagSet("list", flag.ContinueOnError)
	fs.SetOutput(stderr)
	jsonOutput := fs.Bool("json", global.json, "emit JSON")
	tag := fs.String("tag", "", "filter by tag")
	requires := fs.String("requires", "", "filter by requirement")
	if err := fs.Parse(args); err != nil {
		return exitUsage
	}
	cases := registry()
	if err := validateRegistry(cases); err != nil {
		fmt.Fprintln(stderr, err)
		return exitFailed
	}
	filtered := cases[:0]
	for _, c := range cases {
		if (*tag == "" || contains(c.Tags, *tag)) && (*requires == "" || contains(c.Requires, *requires)) {
			filtered = append(filtered, c)
		}
	}
	if *jsonOutput {
		_ = json.NewEncoder(stdout).Encode(filtered)
		return exitOK
	}
	for _, c := range filtered {
		fmt.Fprintf(stdout, "%s  %s  tags=%s\n", c.ID, c.Description, strings.Join(c.Tags, ","))
	}
	return exitOK
}

func runMatrix(args []string, global globalOptions, stdout, stderr io.Writer) int {
	fs := flag.NewFlagSet("matrix", flag.ContinueOnError)
	fs.SetOutput(stderr)
	jsonOutput := fs.Bool("json", global.json, "emit JSON")
	if err := fs.Parse(args); err != nil {
		return exitUsage
	}
	matrix, err := loadFeatureMatrix()
	if err != nil {
		fmt.Fprintln(stderr, err)
		return exitFailed
	}
	if err := validateFeatureCoverage(registry(), matrix); err != nil {
		fmt.Fprintln(stderr, err)
		return exitFailed
	}
	if *jsonOutput {
		_ = json.NewEncoder(stdout).Encode(matrix)
		return exitOK
	}
	type counts struct{ aligned, excluded int }
	byLanguage := map[string]counts{}
	for _, feature := range matrix.Features {
		count := byLanguage[feature.Language]
		if feature.Status == "aligned" {
			count.aligned++
		} else {
			count.excluded++
		}
		byLanguage[feature.Language] = count
	}
	languages := make([]string, 0, len(byLanguage))
	for language := range byLanguage {
		languages = append(languages, language)
	}
	sort.Strings(languages)
	for _, language := range languages {
		count := byLanguage[language]
		fmt.Fprintf(stdout, "%-12s aligned=%-3d intentional-exclusions=%d\n", language, count.aligned, count.excluded)
	}
	return exitOK
}

func runCases(args []string, global globalOptions, stdout, stderr io.Writer) int {
	fs := flag.NewFlagSet("run", flag.ContinueOnError)
	fs.SetOutput(stderr)
	var selects stringList
	fs.Var(&selects, "select", "case ID; repeatable or comma-separated")
	tag := fs.String("tag", "", "filter by tag")
	reportPath := fs.String("report", "", "write aggregate JSON report")
	jsonOutput := fs.Bool("json", global.json, "emit aggregate JSON")
	keep := fs.Bool("keep", false, "retain case workspaces and artifacts")
	timeout := fs.Duration("timeout", 60*time.Second, "timeout per case")
	if err := fs.Parse(args); err != nil {
		return exitUsage
	}
	if *timeout <= 0 {
		fmt.Fprintln(stderr, "--timeout must be greater than zero")
		return exitUsage
	}
	cases, err := selectCases(registry(), selects, *tag)
	if err != nil {
		fmt.Fprintln(stderr, err)
		return exitUsage
	}
	targets := resolveTargets(global)
	probeOriginal(&targets, languagesForCases(cases))
	if !targetsAvailable(targets) {
		for _, t := range targets {
			if !t.Available {
				fmt.Fprintf(stderr, "required target %s unavailable: %s", t.Name, t.Hint)
				if len(t.Missing) > 0 {
					fmt.Fprintf(stderr, " (%s)", strings.Join(t.Missing, ", "))
				}
				fmt.Fprintln(stderr)
			}
		}
		return exitUnavailable
	}
	runRoot, err := os.MkdirTemp("", "diagram-render-e2e-")
	if err != nil {
		fmt.Fprintln(stderr, err)
		return exitFailed
	}
	if !*keep {
		defer os.RemoveAll(runRoot)
	}
	started := time.Now()
	result := report{SchemaVersion: 1, Targets: targets, Total: len(cases), Cases: make([]caseResult, 0, len(cases))}
	for _, c := range cases {
		workspace := filepath.Join(runRoot, "cases", strings.ToLower(c.ID))
		if err := os.MkdirAll(filepath.Join(workspace, "artifacts"), 0o755); err != nil {
			fmt.Fprintln(stderr, err)
			return exitFailed
		}
		ctx, cancel := context.WithTimeout(context.Background(), *timeout)
		caseStarted := time.Now()
		caseResult := executeCase(ctx, &caseEnv{targets: targets, workspace: workspace, keep: *keep}, c)
		cancel()
		caseResult.ID, caseResult.Description = c.ID, c.Description
		caseResult.DurationMS = time.Since(caseStarted).Milliseconds()
		if !*keep {
			caseResult.Artifacts = []string{}
		}
		result.Cases = append(result.Cases, caseResult)
		switch caseResult.Status {
		case "passed":
			result.Passed++
		case "failed":
			result.Failed++
		case "skipped":
			result.Skipped++
		}
	}
	result.DurationMS = time.Since(started).Milliseconds()
	if *reportPath != "" {
		if err := writeJSON(*reportPath, result); err != nil {
			fmt.Fprintf(stderr, "write report: %v\n", err)
			return exitFailed
		}
	}
	if *jsonOutput {
		_ = json.NewEncoder(stdout).Encode(result)
	} else {
		printReport(stdout, result)
		if *keep {
			fmt.Fprintf(stdout, "artifacts: %s\n", runRoot)
		}
	}
	if result.Failed > 0 {
		return exitFailed
	}
	return exitOK
}

func executeCase(ctx context.Context, env *caseEnv, c caseDef) (result caseResult) {
	defer func() {
		if recovered := recover(); recovered != nil {
			result = failed("case-panic", fmt.Sprintf("panic=%v", recovered), truncate(string(debug.Stack())))
		}
	}()
	return c.Run(ctx, env, c)
}

func runParity(ctx context.Context, env *caseEnv, c caseDef) caseResult {
	fixture, err := fixtureFS.ReadFile(c.Fixture)
	if err != nil {
		return failed("fixture-read", err.Error(), "")
	}
	inputDir := filepath.Join(env.workspace, "input")
	if err := os.MkdirAll(inputDir, 0o755); err != nil {
		return failed("workspace", err.Error(), "")
	}
	input := filepath.Join(inputDir, filepath.Base(c.Fixture))
	if err := os.WriteFile(input, fixture, 0o644); err != nil {
		return failed("fixture-materialize", err.Error(), "")
	}
	artifacts := filepath.Join(env.workspace, "artifacts")
	candidatePNG := filepath.Join(artifacts, "diagram-render-rs.png")
	originalPNG := filepath.Join(artifacts, "plot-provider-diagrams.png")
	candidate := targetPath(env.targets, "diagram-render-rs")
	original := targetPath(env.targets, "plot-provider-diagrams")

	candidateRun := runCommand(ctx, env.workspace, candidate, candidateArgs(c, input, candidatePNG, "png")...)
	originalRun := runCommand(ctx, env.workspace, original, originalArgs(c, input, originalPNG, "png")...)
	proof := commandProof("diagram-render-rs", candidateRun) + "\n" + commandProof("plot-provider-diagrams", originalRun)
	if candidateRun.ExitCode != 0 || originalRun.ExitCode != 0 {
		return failed("render-exit", fmt.Sprintf("expected both exits 0, actual diagram-render-rs=%d plot-provider-diagrams=%d", candidateRun.ExitCode, originalRun.ExitCode), proof)
	}
	candidateFact, err := readImageFact(candidatePNG)
	if err != nil {
		return failed("decode-diagram-render-rs", err.Error(), proof)
	}
	originalFact, err := readImageFact(originalPNG)
	if err != nil {
		return failed("decode-plot-provider-diagrams", err.Error(), proof)
	}
	fact := compareImages(candidateFact, originalFact)
	factJSON, _ := json.Marshal(fact)
	proof += "\nnormalized=" + string(factJSON)
	if candidateFact.InkWidth < 2 || candidateFact.InkHeight < 2 || candidateFact.ToneBuckets < 3 || originalFact.InkWidth < 2 || originalFact.InkHeight < 2 || originalFact.ToneBuckets < 3 {
		return failed("nontrivial-artifact", fmt.Sprintf("expected non-empty multi-tone artifacts, actual candidate=%dx%d/%d tones original=%dx%d/%d tones", candidateFact.InkWidth, candidateFact.InkHeight, candidateFact.ToneBuckets, originalFact.InkWidth, originalFact.InkHeight, originalFact.ToneBuckets), proof)
	}
	if fact.AspectDrift > c.Thresholds.MaxAspectDrift {
		return failed("ink-aspect-drift", fmt.Sprintf("oracle=plot-provider-diagrams expected<=%.2f actual=%.3f", c.Thresholds.MaxAspectDrift, fact.AspectDrift), proof)
	}
	if fact.InkCoverageDrift > c.Thresholds.MaxInkDrift {
		return failed("ink-coverage-drift", fmt.Sprintf("oracle=plot-provider-diagrams expected<=%.2f actual=%.3f", c.Thresholds.MaxInkDrift, fact.InkCoverageDrift), proof)
	}
	if fact.MaskIoU < c.Thresholds.MinMaskIoU {
		return failed("visual-mask-iou", fmt.Sprintf("oracle=plot-provider-diagrams expected>=%.2f actual=%.3f", c.Thresholds.MinMaskIoU, fact.MaskIoU), proof)
	}

	retained := []string{candidatePNG, originalPNG}
	if len(c.ExpectedLabels) > 0 || len(c.CandidateLabels) > 0 || len(c.OriginalLabels) > 0 {
		candidateSVG := filepath.Join(artifacts, "diagram-render-rs.svg")
		candidateSVGRun := runCommand(ctx, env.workspace, candidate, candidateArgs(c, input, candidateSVG, "svg")...)
		proof += "\n" + commandProof("diagram-render-rs-svg", candidateSVGRun)
		if candidateSVGRun.ExitCode != 0 {
			return failed("svg-render-exit", fmt.Sprintf("participant=diagram-render-rs expected exit 0 actual=%d", candidateSVGRun.ExitCode), proof)
		}
		candidateText, err := readSVGText(candidateSVG)
		if err != nil {
			return failed("svg-text-diagram-render-rs", err.Error(), proof)
		}
		for _, label := range append(append([]string{}, c.ExpectedLabels...), c.CandidateLabels...) {
			if !containsNormalized(candidateText, label) {
				return failed("semantic-label", fmt.Sprintf("participant=diagram-render-rs label=%q expected in SVG actual=false", label), proof)
			}
		}
		retained = append(retained, candidateSVG)
		if !c.OriginalPNGOnly {
			originalSVG := filepath.Join(artifacts, "plot-provider-diagrams.svg")
			originalSVGRun := runCommand(ctx, env.workspace, original, originalArgs(c, input, originalSVG, "svg")...)
			proof += "\n" + commandProof("plot-provider-diagrams-svg", originalSVGRun)
			if originalSVGRun.ExitCode != 0 {
				return failed("svg-render-exit", fmt.Sprintf("participant=plot-provider-diagrams expected exit 0 actual=%d", originalSVGRun.ExitCode), proof)
			}
			originalText, err := readSVGText(originalSVG)
			if err != nil {
				return failed("svg-text-plot-provider-diagrams", err.Error(), proof)
			}
			for _, label := range append(append([]string{}, c.ExpectedLabels...), c.OriginalLabels...) {
				if !containsNormalized(originalText, label) {
					return failed("semantic-label", fmt.Sprintf("participant=plot-provider-diagrams label=%q expected in SVG actual=false", label), proof)
				}
			}
			retained = append(retained, originalSVG)
		}
		proof += fmt.Sprintf("\nsemantic_labels=%q candidate_labels=%q original_labels=%q original_png_only=%t", c.ExpectedLabels, c.CandidateLabels, c.OriginalLabels, c.OriginalPNGOnly)
	}
	if !env.keep {
		retained = []string{}
	}
	return caseResult{Status: "passed", Output: proof, Artifacts: retained}
}

func runValidation(ctx context.Context, env *caseEnv, c caseDef) caseResult {
	fixture, err := fixtureFS.ReadFile(c.Fixture)
	if err != nil {
		return failed("fixture-read", err.Error(), "")
	}
	inputDir := filepath.Join(env.workspace, "input")
	if err := os.MkdirAll(inputDir, 0o755); err != nil {
		return failed("workspace", err.Error(), "")
	}
	input := filepath.Join(inputDir, filepath.Base(c.Fixture))
	if err := os.WriteFile(input, fixture, 0o644); err != nil {
		return failed("fixture-materialize", err.Error(), "")
	}
	candidateRun := runCommand(ctx, env.workspace, targetPath(env.targets, "diagram-render-rs"), candidateArgs(c, input, filepath.Join(env.workspace, "artifacts", "candidate.svg"), "svg")...)
	originalRun := runCommand(ctx, env.workspace, targetPath(env.targets, "plot-provider-diagrams"), originalArgs(c, input, filepath.Join(env.workspace, "artifacts", "original.svg"), "svg")...)
	proof := commandProof("diagram-render-rs", candidateRun) + "\n" + commandProof("plot-provider-diagrams", originalRun)
	if candidateRun.ExitCode == 0 || originalRun.ExitCode == 0 {
		return failed("invalid-input-exit", fmt.Sprintf("expected both exits nonzero, actual diagram-render-rs=%d plot-provider-diagrams=%d", candidateRun.ExitCode, originalRun.ExitCode), proof)
	}
	return caseResult{Status: "passed", Output: proof, Artifacts: []string{}}
}

func candidateArgs(c caseDef, input, output, format string) []string {
	return []string{input, "--format", c.Language, "--output", output, "--output-format", format, "--scale", "1", "--quiet"}
}

func originalArgs(c caseDef, input, output, format string) []string {
	args := []string{"render", c.Language, input, "--output", output, "--format", format}
	if c.OriginalView != "" {
		args = append(args, "--view", c.OriginalView)
	}
	return args
}

func failed(assertion, actual, proof string) caseResult {
	return caseResult{Status: "failed", Error: fmt.Sprintf("assertion=%s %s", assertion, actual), Output: proof, Artifacts: []string{}}
}

func resolveTargets(global globalOptions) []target {
	root := repoRoot()
	candidate := resolveBinary(global.diagramRender, "DIAGRAM_RENDER_BIN", []string{
		filepath.Join(root, "target", "debug", "diagram-render-rs"),
		filepath.Join(root, "target", "release", "diagram-render-rs"),
	}, "diagram-render-rs")
	originalCandidates := syncCandidates("plot-provider-diagrams")
	originalCandidates = append(originalCandidates, filepath.Join(root, "..", "plot-provider-diagrams", "plot-provider-diagrams"))
	original := resolveBinary(global.original, "PLOT_PROVIDER_DIAGRAMS_BIN", originalCandidates, "plot-provider-diagrams")
	return []target{
		{Name: "diagram-render-rs", Kind: "bin", Path: candidate, Available: candidate != "", Required: true, Hint: "cargo build or pass --diagram-render PATH"},
		{Name: "plot-provider-diagrams", Kind: "oracle", Path: original, Available: original != "", Required: true, Hint: "install/build plot-provider-diagrams or pass --original PATH"},
	}
}

func probeOriginal(targets *[]target, requiredLanguages []string) {
	index := -1
	for i := range *targets {
		if (*targets)[i].Name == "plot-provider-diagrams" {
			index = i
			break
		}
	}
	if index < 0 || !(*targets)[index].Available {
		return
	}
	workspace, err := os.MkdirTemp("", "diagram-render-doctor-")
	if err != nil {
		(*targets)[index].Available = false
		(*targets)[index].Missing = []string{err.Error()}
		return
	}
	defer os.RemoveAll(workspace)
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	result := runCommand(ctx, workspace, (*targets)[index].Path, "doctor", "--json")
	if result.ExitCode != 0 {
		(*targets)[index].Available = false
		(*targets)[index].Missing = []string{"doctor failed: " + firstNonEmpty(result.Stderr, result.Stdout)}
		return
	}
	var document originalDoctor
	if err := json.Unmarshal([]byte(result.Stdout), &document); err != nil {
		(*targets)[index].Available = false
		(*targets)[index].Missing = []string{"invalid doctor JSON: " + err.Error()}
		return
	}
	required := map[string]bool{}
	for _, language := range requiredLanguages {
		required[language] = true
	}
	ready := map[string]bool{}
	missing := []string{}
	for _, item := range document.Items {
		if !required[item.Backend] {
			continue
		}
		if item.Available {
			ready[item.Backend] = true
		} else {
			missing = append(missing, item.Backend+":"+strings.Join(item.Missing, ","))
		}
	}
	for backend := range required {
		if !ready[backend] && !hasPrefix(missing, backend+":") {
			missing = append(missing, backend+":not reported by doctor")
		}
	}
	sort.Strings(missing)
	capabilities := make([]string, 0, len(ready))
	for backend := range ready {
		capabilities = append(capabilities, backend)
	}
	sort.Strings(capabilities)
	(*targets)[index].Capabilities = capabilities
	(*targets)[index].Missing = missing
	if len(missing) > 0 {
		(*targets)[index].Available = false
	}
}

func languagesForCases(cases []caseDef) []string {
	seen := map[string]bool{}
	languages := []string{}
	for _, c := range cases {
		if !seen[c.Language] {
			seen[c.Language] = true
			languages = append(languages, c.Language)
		}
	}
	sort.Strings(languages)
	return languages
}

func resolveBinary(explicit, envName string, candidates []string, pathName string) string {
	if explicit != "" {
		return executablePath(explicit)
	}
	if value := os.Getenv(envName); value != "" {
		return executablePath(value)
	}
	for _, candidate := range candidates {
		if path := executablePath(candidate); path != "" {
			return path
		}
	}
	if path, err := exec.LookPath(pathName); err == nil {
		absolute, _ := filepath.Abs(path)
		return absolute
	}
	return ""
}

func syncCandidates(name string) []string {
	home, _ := os.UserHomeDir()
	osName := runtime.GOOS
	if osName == "darwin" {
		osName = "macos"
	}
	archName := runtime.GOARCH
	if archName == "arm64" {
		archName = "arm64"
	} else if archName == "amd64" {
		archName = "x86"
	}
	paths := []string{}
	if directory := os.Getenv("SYNC_BIN_DIR"); directory != "" {
		paths = append(paths, filepath.Join(directory, name))
	}
	if home != "" {
		paths = append(paths,
			filepath.Join(home, "sync", osName+"-"+archName+"-bin", name),
			filepath.Join(home, "sync", "bin", name),
		)
	}
	return paths
}

func executablePath(path string) string {
	info, err := os.Stat(path)
	if err != nil || info.IsDir() || info.Mode()&0o111 == 0 {
		return ""
	}
	absolute, _ := filepath.Abs(path)
	return absolute
}

func repoRoot() string {
	dir, _ := os.Getwd()
	for current := dir; current != filepath.Dir(current); current = filepath.Dir(current) {
		data, err := os.ReadFile(filepath.Join(current, "Cargo.toml"))
		if err == nil && bytes.Contains(data, []byte("name = \"diagram-render-rs\"")) {
			return current
		}
	}
	return filepath.Dir(dir)
}

func runCommand(ctx context.Context, workspace, bin string, args ...string) execResult {
	argv := append([]string{bin}, args...)
	cmd := exec.CommandContext(ctx, bin, args...)
	home := filepath.Join(workspace, "home")
	_ = os.MkdirAll(home, 0o755)
	cmd.Env = deterministicEnv(home)
	var stdout, stderr bytes.Buffer
	cmd.Stdout, cmd.Stderr = &stdout, &stderr
	started := time.Now()
	err := cmd.Run()
	exitCode := 0
	if err != nil {
		exitCode = -1
		var exitErr *exec.ExitError
		if errors.As(err, &exitErr) {
			exitCode = exitErr.ExitCode()
		}
		if errors.Is(ctx.Err(), context.DeadlineExceeded) {
			exitCode = -2
		}
	}
	return execResult{Argv: argv, ExitCode: exitCode, DurationMS: time.Since(started).Milliseconds(), Stdout: truncate(stdout.String()), Stderr: truncate(stderr.String())}
}

func deterministicEnv(home string) []string {
	allowed := []string{"PATH", "JAVA_HOME", "GRAPHVIZ_DOT", "SystemRoot", "WINDIR", "TMPDIR", "FONTCONFIG_PATH"}
	env := []string{"HOME=" + home, "TZ=UTC", "LANG=C.UTF-8", "LC_ALL=C.UTF-8"}
	for _, key := range allowed {
		if value, ok := os.LookupEnv(key); ok {
			env = append(env, key+"="+value)
		}
	}
	sort.Strings(env)
	return env
}

func commandProof(name string, result execResult) string {
	return fmt.Sprintf("participant=%s argv=%q exit=%d duration_ms=%d stdout=%q stderr=%q", name, result.Argv, result.ExitCode, result.DurationMS, result.Stdout, result.Stderr)
}

func readImageFact(path string) (imageFact, error) {
	file, err := os.Open(path)
	if err != nil {
		return imageFact{}, err
	}
	defer file.Close()
	img, err := png.Decode(file)
	if err != nil {
		return imageFact{}, err
	}
	bounds := img.Bounds()
	minX, minY, maxX, maxY := bounds.Max.X, bounds.Max.Y, bounds.Min.X-1, bounds.Min.Y-1
	toneBuckets := map[uint16]struct{}{}
	for y := bounds.Min.Y; y < bounds.Max.Y; y++ {
		for x := bounds.Min.X; x < bounds.Max.X; x++ {
			toneBuckets[toneBucket(img, x, y)] = struct{}{}
			if pixelInk(img, x, y) {
				minX = min(minX, x)
				minY = min(minY, y)
				maxX = max(maxX, x)
				maxY = max(maxY, y)
			}
		}
	}
	fact := imageFact{Width: bounds.Dx(), Height: bounds.Dy(), ToneBuckets: len(toneBuckets), Mask: make([]float64, gridSize*gridSize)}
	if maxX < minX || maxY < minY {
		return fact, nil
	}
	fact.InkWidth = maxX - minX + 1
	fact.InkHeight = maxY - minY + 1
	fact.AspectRatio = float64(fact.InkWidth) / float64(fact.InkHeight)
	const samples = 4
	for gy := 0; gy < gridSize; gy++ {
		for gx := 0; gx < gridSize; gx++ {
			ink := 0
			for sy := 0; sy < samples; sy++ {
				for sx := 0; sx < samples; sx++ {
					nx := (float64(gx) + (float64(sx)+0.5)/samples) / gridSize
					ny := (float64(gy) + (float64(sy)+0.5)/samples) / gridSize
					x := minX + min(int(nx*float64(fact.InkWidth)), fact.InkWidth-1)
					y := minY + min(int(ny*float64(fact.InkHeight)), fact.InkHeight-1)
					if pixelInk(img, x, y) {
						ink++
					}
				}
			}
			fact.Mask[gy*gridSize+gx] = float64(ink) / float64(samples*samples)
		}
	}
	fact.Mask = softenMask(fact.Mask)
	for _, value := range fact.Mask {
		fact.InkCoverage += value
	}
	fact.InkCoverage /= float64(len(fact.Mask))
	return fact, nil
}

func pixelInk(img image.Image, x, y int) bool {
	r, g, b, a := img.At(x, y).RGBA()
	alpha := float64(a) / 65535.0
	red := float64(r)/65535.0*alpha + (1 - alpha)
	green := float64(g)/65535.0*alpha + (1 - alpha)
	blue := float64(b)/65535.0*alpha + (1 - alpha)
	luma := 0.2126*red + 0.7152*green + 0.0722*blue
	return luma < 0.97
}

func toneBucket(img image.Image, x, y int) uint16 {
	r, g, b, a := img.At(x, y).RGBA()
	alpha := float64(a) / 65535.0
	red := uint16(math.Round((float64(r)/65535.0*alpha + (1 - alpha)) * 255))
	green := uint16(math.Round((float64(g)/65535.0*alpha + (1 - alpha)) * 255))
	blue := uint16(math.Round((float64(b)/65535.0*alpha + (1 - alpha)) * 255))
	return (red>>5)<<6 | (green>>5)<<3 | blue>>5
}

func softenMask(mask []float64) []float64 {
	softened := make([]float64, len(mask))
	for y := 0; y < gridSize; y++ {
		for x := 0; x < gridSize; x++ {
			value := 0.0
			for dy := -1; dy <= 1; dy++ {
				for dx := -1; dx <= 1; dx++ {
					nx, ny := x+dx, y+dy
					if nx >= 0 && nx < gridSize && ny >= 0 && ny < gridSize {
						value = math.Max(value, mask[ny*gridSize+nx])
					}
				}
			}
			softened[y*gridSize+x] = value
		}
	}
	return softened
}

func compareImages(actual, oracle imageFact) parityFact {
	var intersection, union float64
	for i := range actual.Mask {
		intersection += math.Min(actual.Mask[i], oracle.Mask[i])
		union += math.Max(actual.Mask[i], oracle.Mask[i])
	}
	iou := 1.0
	if union > 0 {
		iou = intersection / union
	}
	return parityFact{
		DiagramRender:    actual,
		Original:         oracle,
		AspectDrift:      relativeDrift(actual.AspectRatio, oracle.AspectRatio),
		InkCoverageDrift: relativeDrift(actual.InkCoverage, oracle.InkCoverage),
		MaskIoU:          iou,
	}
}

func relativeDrift(actual, expected float64) float64 {
	if expected == 0 {
		if actual == 0 {
			return 0
		}
		return 1
	}
	return math.Abs(actual-expected) / math.Abs(expected)
}

func readSVGText(path string) (string, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer file.Close()
	decoder := xml.NewDecoder(file)
	depth := 0
	parts := []string{}
	for {
		token, err := decoder.Token()
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			return "", err
		}
		switch value := token.(type) {
		case xml.StartElement:
			if depth > 0 {
				depth++
			} else if value.Name.Local == "text" {
				depth = 1
			}
		case xml.EndElement:
			if depth > 0 {
				depth--
			}
		case xml.CharData:
			if depth > 0 {
				parts = append(parts, string(value))
			}
		}
	}
	return strings.Join(strings.Fields(strings.Join(parts, " ")), " "), nil
}

func containsNormalized(haystack, needle string) bool {
	normalize := func(value string) string {
		return strings.ToLower(strings.Join(strings.Fields(value), " "))
	}
	return strings.Contains(normalize(haystack), normalize(needle))
}

func selectCases(cases []caseDef, selectors []string, tag string) ([]caseDef, error) {
	if err := validateRegistry(cases); err != nil {
		return nil, err
	}
	wanted := map[string]bool{}
	for _, selector := range selectors {
		for _, id := range strings.Split(selector, ",") {
			if id = strings.TrimSpace(id); id != "" {
				wanted[strings.ToUpper(id)] = true
			}
		}
	}
	selected := []caseDef{}
	filteringByID := len(wanted) > 0
	matched := map[string]bool{}
	for _, c := range cases {
		if filteringByID && !wanted[c.ID] {
			continue
		}
		if tag != "" && !contains(c.Tags, tag) {
			continue
		}
		selected = append(selected, c)
		matched[c.ID] = true
	}
	if filteringByID && len(matched) != len(wanted) {
		ids := make([]string, 0, len(wanted)-len(matched))
		for id := range wanted {
			if !matched[id] {
				ids = append(ids, id)
			}
		}
		sort.Strings(ids)
		return nil, fmt.Errorf("unknown case selection: %s", strings.Join(ids, ","))
	}
	if len(selected) == 0 {
		return nil, errors.New("selection matched no cases")
	}
	return selected, nil
}

type stringList []string

func (s *stringList) String() string         { return strings.Join(*s, ",") }
func (s *stringList) Set(value string) error { *s = append(*s, value); return nil }

func targetsAvailable(targets []target) bool {
	for _, t := range targets {
		if t.Required && !t.Available {
			return false
		}
	}
	return true
}

func targetPath(targets []target, name string) string {
	for _, t := range targets {
		if t.Name == name {
			return t.Path
		}
	}
	return ""
}

func contains(values []string, wanted string) bool {
	for _, value := range values {
		if value == wanted {
			return true
		}
	}
	return false
}

func hasPrefix(values []string, prefix string) bool {
	for _, value := range values {
		if strings.HasPrefix(value, prefix) {
			return true
		}
	}
	return false
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if value != "" {
			return value
		}
	}
	return ""
}

func truncate(value string) string {
	const limit = 2000
	if len(value) <= limit {
		return value
	}
	return value[:limit] + "...[truncated]"
}

func writeJSON(path string, value any) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil && filepath.Dir(path) != "." {
		return err
	}
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, append(data, '\n'), 0o644)
}

func printReport(w io.Writer, result report) {
	for _, c := range result.Cases {
		fmt.Fprintf(w, "%s %-7s %s\n", c.ID, strings.ToUpper(c.Status), c.Description)
		if c.Status == "failed" {
			fmt.Fprintf(w, "  %s\n  proof: %s\n", c.Error, c.Output)
		}
	}
	fmt.Fprintf(w, "\n%d passed, %d failed, %d skipped\n", result.Passed, result.Failed, result.Skipped)
}
