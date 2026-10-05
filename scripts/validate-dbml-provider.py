#!/usr/bin/env python3
"""Validate real provider output against the closed authored/receipt schemas.
Requires Python 3 and jsonschema; does not install tools or use the network.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import xml.etree.ElementTree as ET
from jsonschema import Draft202012Validator

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("provider", type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
provider = args.provider.resolve(strict=True)

def read(path):
    return json.loads(path.read_text())

def validator(name):
    schema = read(root / "schemas" / name)
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)

source_validator = validator("dbml-authored-v1.schema.json")
receipt_validator = validator("dbml-render-receipt-v1.schema.json")
source = root / "examples/dbml-authored/users-posts.json"
source_validator.validate(read(source))
input_sha = hashlib.sha256(source.read_bytes()).hexdigest()
command = read(root / "assets/dbml-render-svg-command-v1.json")
describe = json.loads(subprocess.check_output([provider, "describe", "--json"]))
assert describe["commands"][0] == command
command_validator = Draft202012Validator(command["input_schema"])
checks = 0
with tempfile.TemporaryDirectory(prefix="dbml-schema-gate-") as temporary:
    folder = Path(temporary)
    for theme in command["input_schema"]["properties"]["theme"]["enum"]:
        for background in [None, "#123aEF"]:
            output = folder / "figure.svg"
            receipt = folder / "receipt.json"
            params = dict(input=str(source), resource_pins={"input": input_sha}, output=str(output), receipt=str(receipt), theme=theme)
            if background is not None:
                params["background"] = background
            command_validator.validate(params)
            argv = [str(provider), "render-svg"]
            for flag in command["cli_spec"]["flags"]:
                if flag["name"] in params:
                    value = params[flag["name"]]
                    argv.extend([flag["flag"], json.dumps(value) if flag["kind"] == "json" else value])
            result = subprocess.run(argv, capture_output=True, check=True)
            assert not result.stdout and not result.stderr
            value = read(receipt)
            receipt_validator.validate(value)
            core = value["artifact_receipt"]
            assert core["inputs"] == [{"role": "input", "sha256": input_sha, "bytes": source.stat().st_size}]
            assert core["primary"]["sha256"] == hashlib.sha256(output.read_bytes()).hexdigest()
            assert core["primary"]["bytes"] == output.stat().st_size
            assert value["options"] == {"theme": theme, "background": background, "warning_policy": "deny"}
            assert temporary not in receipt.read_text()
            svg = ET.fromstring(output.read_bytes())
            assert svg.tag == "{http://www.w3.org/2000/svg}svg"
            for element in svg.iter():
                assert element.tag.rsplit("}", 1)[-1] not in ["image", "script", "foreignObject", "use"]
                assert not any(key.rsplit("}", 1)[-1] == "href" for key in element.attrib)
                assert not any("url(" in value for value in element.attrib.values())
            checks += 1
print(f"PASS: {checks} exact descriptor-driven renders, closed schemas, receipt byte relations and static SVG checks")
