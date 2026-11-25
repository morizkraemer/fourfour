#!/usr/bin/env python3
import argparse
import glob
import hashlib
import json
import os
import re
import subprocess
import sys
import tomllib

TARGET = "aarch64-apple-darwin"
ROOT_PACKAGE = "pioneer-test-ui"
LICENSE_FILE_RE = re.compile(r"^(licen[cs]e|copying|notice|unlicense|copyright)([-_. ].*)?$", re.I)
TOKEN_RE = re.compile(r"\(|\)|/|[A-Za-z0-9.+\-]+")

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
HERE = os.path.join(REPO, "scripts", "release")
STANDARD_DIR = os.path.join(HERE, "licenses")
EXTRAS_PATH = os.path.join(HERE, "third-party-extras.json")
REGISTRY_SRC = os.path.expanduser("~/.cargo/registry/src")


class Parser:
    def __init__(self, expr):
        self.tokens = TOKEN_RE.findall(expr)
        self.pos = 0

    def peek(self):
        return self.tokens[self.pos] if self.pos < len(self.tokens) else None

    def take(self):
        token = self.peek()
        self.pos += 1
        return token

    def parse(self):
        node = self.parse_or()
        if self.peek() is not None:
            raise ValueError(f"unparsed token {self.peek()!r}")
        return node

    def parse_or(self):
        nodes = [self.parse_and()]
        while self.peek() in ("OR", "/"):
            self.take()
            nodes.append(self.parse_and())
        return ("or", nodes) if len(nodes) > 1 else nodes[0]

    def parse_and(self):
        nodes = [self.parse_atom()]
        while self.peek() == "AND":
            self.take()
            nodes.append(self.parse_atom())
        return ("and", nodes) if len(nodes) > 1 else nodes[0]

    def parse_atom(self):
        token = self.take()
        if token == "(":
            node = self.parse_or()
            if self.take() != ")":
                raise ValueError("missing )")
            return node
        if token is None or token in (")", "AND", "OR", "WITH", "/"):
            raise ValueError(f"unexpected token {token!r}")
        if self.peek() == "WITH":
            self.take()
            self.take()
        return ("id", token)


def ids_of(node):
    if node[0] == "id":
        return [node[1]]
    return [i for child in node[1] for i in ids_of(child)]


def is_allowed(node, allowed):
    if node[0] == "id":
        return node[1] in allowed
    results = [is_allowed(child, allowed) for child in node[1]]
    return any(results) if node[0] == "or" else all(results)


def canonical(expression):
    spaced = re.sub(r"\s*/\s*", " OR ", expression.strip())
    if "(" in spaced or " AND " in spaced:
        return spaced
    return " OR ".join(sorted(set(re.split(r"\s+OR\s+", spaced))))


def read_text(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        text = handle.read()
    lines = [line.rstrip() for line in text.replace("\r\n", "\n").replace("\r", "\n").split("\n")]
    return "\n".join(lines).strip("\n")


def find_registry_dir(name, version):
    matches = glob.glob(os.path.join(REGISTRY_SRC, "*", f"{name}-{version}"))
    if len(matches) != 1:
        raise SystemExit(f"FAIL: expected exactly one registry source for {name} {version}, found {len(matches)}")
    return matches[0]


def license_files(directory):
    names = [n for n in os.listdir(directory) if os.path.isfile(os.path.join(directory, n)) and LICENSE_FILE_RE.match(n)]
    return sorted(names)


def standard_text(spdx_id, holder):
    path = os.path.join(STANDARD_DIR, f"{spdx_id}.txt")
    if not os.path.isfile(path):
        raise SystemExit(f"FAIL: no standard text for {spdx_id}; add {path}")
    return read_text(path).replace("{holder}", holder)


def run_metadata():
    command = ["cargo", "metadata", "--format-version", "1", "--locked", "--offline", "--filter-platform", TARGET]
    result = subprocess.run(command, cwd=REPO, capture_output=True, text=True)
    if result.returncode != 0:
        raise SystemExit(f"FAIL: cargo metadata\n{result.stderr}")
    return json.loads(result.stdout)


def linked_crates(metadata):
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    root = next(p["id"] for p in metadata["packages"] if p["name"] == ROOT_PACKAGE)
    seen = set()
    stack = [root]
    while stack:
        package_id = stack.pop()
        if package_id in seen:
            continue
        seen.add(package_id)
        for dep in nodes[package_id]["deps"]:
            if any(kind["kind"] is None for kind in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    crates = {}
    for package_id in seen:
        package = packages[package_id]
        if package["source"] is None:
            continue
        crates[(package["name"], package["version"])] = {
            "directory": os.path.dirname(package["manifest_path"]),
            "license": package["license"],
            "authors": package["authors"],
        }
    return crates


def lock_graph():
    with open(os.path.join(REPO, "Cargo.lock"), "rb") as handle:
        lock = tomllib.load(handle)
    versions = {}
    deps = {}
    for package in lock["package"]:
        versions.setdefault(package["name"], []).append(package["version"])
        deps[package["name"]] = [d.split()[0] for d in package.get("dependencies", [])]
    return versions, deps


def feature_extra_crates(extras, known_names):
    versions, deps = lock_graph()
    prune = set(extras["prune"])
    seen = set()
    stack = list(extras["roots"])
    while stack:
        name = stack.pop()
        if name in seen or name in known_names or name in prune:
            continue
        seen.add(name)
        stack.extend(deps[name])
    crates = {}
    for name in seen:
        if len(versions[name]) != 1:
            raise SystemExit(f"FAIL: feature extra {name} has several locked versions; extend the script")
        version = versions[name][0]
        directory = find_registry_dir(name, version)
        with open(os.path.join(directory, "Cargo.toml"), "rb") as handle:
            package = tomllib.load(handle)["package"]
        crates[(name, version)] = {
            "directory": directory,
            "license": package.get("license"),
            "authors": package.get("authors", []),
        }
    return crates


def collect_crate_entries(crates, overrides, allowed):
    entries = []
    failures = []
    for (name, version), info in sorted(crates.items()):
        key = f"{name}@{version}"
        declared = overrides.get(key) or info["license"]
        if not declared:
            failures.append(f"{key}: no license field and no override")
            continue
        expression = canonical(declared)
        try:
            tree = Parser(expression).parse()
        except ValueError as error:
            failures.append(f"{key}: cannot parse licence {expression!r} ({error})")
            continue
        if not is_allowed(tree, allowed):
            failures.append(f"{key}: licence {expression!r} is outside the allow list")
        entries.append({
            "name": name,
            "version": version,
            "expression": expression,
            "ids": sorted(set(ids_of(tree))),
            "directory": info["directory"],
            "authors": info["authors"],
        })
    return entries, failures


def crate_texts(entry, fallbacks):
    files = license_files(entry["directory"])
    label = f"{entry['name']} {entry['version']}"
    if files:
        return [(label, read_text(os.path.join(entry["directory"], f))) for f in files]
    holder = ", ".join(a.split("<")[0].strip() for a in entry["authors"]) or f"the {entry['name']} contributors"
    fallbacks.append(label)
    return [(label, standard_text(i, holder)) for i in entry["ids"]]


def component_texts(component):
    texts = []
    for source in component["texts"]:
        if "repo_file" in source:
            texts.append(read_text(os.path.join(REPO, source["repo_file"])))
        elif "cargo_crate_file" in source:
            crate, relative = source["cargo_crate_file"]
            versions = sorted(glob.glob(os.path.join(REGISTRY_SRC, "*", f"{crate}-*")))
            if not versions:
                raise SystemExit(f"FAIL: {crate} not in cargo registry for {component['name']}")
            texts.append(read_text(os.path.join(versions[-1], relative)))
        else:
            texts.append(declared_or_standard_text(source))
    return texts


def declared_or_standard_text(source):
    if "node_module" in source:
        directory = os.path.join(REPO, "app", "node_modules", *source["node_module"].split("/"))
        if not os.path.isdir(directory):
            raise SystemExit(f"FAIL: {directory} is missing; run npm ci in app first")
        files = license_files(directory)
        if files:
            return read_text(os.path.join(directory, files[0]))
    return standard_text(source["standard"], source["holder"])


def render(entries, components, extras, fallbacks):
    by_expression = {}
    for entry in entries:
        by_expression.setdefault(entry["expression"], []).append(entry)

    blocks = {}

    def add_block(text, user):
        digest = hashlib.sha256(text.encode()).hexdigest()
        blocks.setdefault(digest, {"text": text, "users": set()})["users"].add(user)

    for entry in entries:
        for user, text in crate_texts(entry, []):
            add_block(text, user)
    for component in components:
        for text in component_texts(component):
            add_block(text, component["name"])

    out = []
    out.append("fourfour third-party notices")
    out.append("=" * 28)
    out.append("")
    out.append("fourfour includes the software listed below. Each component is used under the")
    out.append("licence shown. Full licence texts are in section 4.")
    out.append("")
    out.append(f"Rust crates linked into the macOS app (target {TARGET}, release build with")
    out.append("the coreml feature, no dev-dependencies, no dev-bins): " + str(len(entries)))
    out.append("")
    out.append("1. Rust crates by licence")
    out.append("-" * 25)
    out.append("")
    for expression in sorted(by_expression):
        group = by_expression[expression]
        out.append(f"{expression} ({len(group)})")
        for entry in group:
            out.append(f"  {entry['name']} {entry['version']}")
        out.append("")
    out.append("2. Other components")
    out.append("-" * 19)
    out.append("")
    for component in sorted(components, key=lambda c: c["name"].lower()):
        out.append(f"{component['name']}")
        out.append(f"  Version: {component['version']}")
        out.append(f"  Licence: {component['license']}")
        out.append(f"  Source:  {component['url']}")
        if component.get("note"):
            out.append(f"  Note:    {component['note']}")
        out.append("")
    out.append("3. Source availability (MPL-2.0)")
    out.append("-" * 32)
    out.append("")
    for spdx_id, note in sorted(extras["source_availability_licenses"].items()):
        affected = [f"{e['name']} {e['version']}" for e in entries if spdx_id in e["ids"] and e["expression"] == spdx_id]
        out.append(f"{spdx_id}: {note}")
        out.append("  Crates: " + ", ".join(affected))
        out.append("")
    if fallbacks:
        out.append("Standard licence text substituted (crate ships no licence file): " + ", ".join(sorted(fallbacks)))
        out.append("")
    out.append("4. Licence texts")
    out.append("-" * 16)
    out.append("")
    ordered = sorted(blocks.values(), key=lambda b: (sorted(b["users"])[0].lower(), b["text"]))
    for block in ordered:
        users = sorted(block["users"], key=str.lower)
        out.append("=" * 78)
        out.append("Applies to: " + ", ".join(users))
        out.append("=" * 78)
        out.append("")
        out.append(block["text"])
        out.append("")
    return "\n".join(out).rstrip("\n") + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output")
    parser.add_argument("--inject-fake-license")
    args = parser.parse_args()

    with open(EXTRAS_PATH, encoding="utf-8") as handle:
        extras = json.load(handle)
    allowed = set(extras["allowed_licenses"])

    crates = linked_crates(run_metadata())
    known_names = {name for name, _ in crates}
    extra_crates = feature_extra_crates(extras["feature_extras"], known_names)
    crates.update(extra_crates)
    if args.inject_fake_license:
        crates[("fake-injected-crate", "0.0.0")] = {
            "directory": REPO,
            "license": args.inject_fake_license,
            "authors": [],
        }

    entries, failures = collect_crate_entries(crates, extras["license_overrides"], allowed)
    if failures:
        print("FAIL: licence check", file=sys.stderr)
        for failure in failures:
            print("  " + failure, file=sys.stderr)
        sys.exit(1)

    fallbacks = []
    for entry in entries:
        crate_texts(entry, fallbacks)
    text = render(entries, extras["components"], extras, fallbacks)
    with open(args.output, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)

    histogram = {}
    for entry in entries:
        histogram[entry["expression"]] = histogram.get(entry["expression"], 0) + 1
    print(f"crates: {len(entries)} (feature extras: {len(extra_crates)})")
    for expression, count in sorted(histogram.items(), key=lambda kv: (-kv[1], kv[0])):
        print(f"  {count:4d}  {expression}")
    print(f"standard-text fallbacks: {len(fallbacks)}")
    for label in sorted(fallbacks):
        print(f"  {label}")


main()
