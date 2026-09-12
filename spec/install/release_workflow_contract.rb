#!/usr/bin/env ruby
# frozen_string_literal: true

require "yaml"
require "digest"
require "fileutils"
require "json"
require "open3"
require "tmpdir"

ROOT = File.expand_path("../..", __dir__)

def check(condition, message)
  raise message unless condition
end

def workflow(path)
  YAML.load_file(File.join(ROOT, path))
end

def step(job, name)
  job.fetch("steps").find { |candidate| candidate["name"] == name } ||
    raise("missing step #{name.inspect}")
end

def run_bash(script, environment = {}, directory = ROOT)
  Open3.capture2e(environment, "bash", "-c", script, chdir: directory)
end

build = workflow(".github/workflows/release-build.yml")
workflow_call = build.fetch(true).fetch("workflow_call")
inputs = workflow_call.fetch("inputs")
check(inputs.keys.sort == %w[extension runner target use_cross], "workflow_call inputs changed")
check(inputs["target"] == {"required" => true, "type" => "string"}, "target input changed")
check(inputs["runner"] == {"required" => true, "type" => "string"}, "runner input changed")
check(inputs["use_cross"] == {"required" => true, "type" => "boolean"}, "use_cross input changed")
check(inputs["extension"] == {"required" => true, "type" => "string"}, "extension input changed")

build_job = build.fetch("jobs").fetch("build")
check(
  build_job.fetch("permissions") == {
    "contents" => "read",
    "id-token" => "write",
    "attestations" => "write",
    "artifact-metadata" => "write"
  },
  "reusable build permissions changed"
)
validation = step(build_job, "Validate release target").fetch("run")
[
  "x86_64-unknown-linux-musl\\|ubuntu-latest\\|false\\|",
  "aarch64-unknown-linux-musl\\|ubuntu-latest\\|true\\|",
  "x86_64-apple-darwin\\|macos-latest\\|false\\|",
  "aarch64-apple-darwin\\|macos-latest\\|false\\|",
  "x86_64-pc-windows-msvc\\|windows-latest\\|false\\|.exe"
].each do |tuple|
  check(validation.include?(tuple), "release tuple allowlist is missing #{tuple.inspect}")
end
check(build_job.fetch("steps").first.fetch("name") == "Validate release target", "release tuple validation must run before checkout")
allowed_tuples = [
  %w[x86_64-unknown-linux-musl ubuntu-latest false] << "",
  %w[aarch64-unknown-linux-musl ubuntu-latest true] << "",
  %w[x86_64-apple-darwin macos-latest false] << "",
  %w[aarch64-apple-darwin macos-latest false] << "",
  %w[x86_64-pc-windows-msvc windows-latest false .exe]
]
allowed_tuples.each do |target, runner, use_cross, extension|
  _, status = run_bash(validation, {
    "TARGET" => target, "RELEASE_RUNNER" => runner,
    "USE_CROSS" => use_cross, "EXTENSION" => extension
  })
  check(status.success?, "approved release tuple was rejected: #{[target, runner, use_cross, extension].inspect}")
end
[
  ["unknown-target", "ubuntu-latest", "false", ""],
  [allowed_tuples[0][0], "self-hosted", "false", ""],
  [allowed_tuples[0][0], "ubuntu-latest", "true", ""],
  [allowed_tuples[0][0], "ubuntu-latest", "false", ".exe"],
  [allowed_tuples[4][0], "windows-latest", "false", ""]
].each do |target, runner, use_cross, extension|
  _, status = run_bash(validation, {
    "TARGET" => target, "RELEASE_RUNNER" => runner,
    "USE_CROSS" => use_cross, "EXTENSION" => extension
  })
  check(!status.success?, "unsupported release tuple was accepted: #{[target, runner, use_cross, extension].inspect}")
end
uses = build_job.fetch("steps").map { |candidate| candidate["uses"] }.compact
check(uses.include?("actions/attest@v4"), "actions/attest must stay pinned to v4")
check(uses.include?("sigstore/cosign-installer@v4.1.2"), "cosign installer pin changed")
package = step(build_job, "Package archive and checksum").fetch("run")
check(package.include?("tools/release/package.sh"), "reusable build bypasses package.sh")
sign = step(build_job, "Sign archive").fetch("run")
check(sign.include?("cosign sign-blob --yes --bundle"), "archive is not signed with a bundle")
attest = step(build_job, "Attest archive provenance")
check(attest["uses"] == "actions/attest@v4", "provenance action pin changed")
check(attest.fetch("with").keys == ["subject-path"], "provenance subject inputs changed")
uploaded = step(build_job, "Upload release assets")
check(uploaded.fetch("with").fetch("if-no-files-found") == "error", "missing assets may be ignored")
archive_pattern = 'prim-${{ inputs.target }}.tar.gz'
check(
  uploaded.fetch("with").fetch("path").lines.map(&:strip) ==
    [archive_pattern, "#{archive_pattern}.sha256", "#{archive_pattern}.sigstore.json", "#{archive_pattern}.provenance.sigstore.json"],
  "reusable workflow does not upload exactly four assets"
)
check(attest.fetch("id") == "provenance", "provenance output source changed")
check(attest.fetch("with").fetch("subject-path") == archive_pattern, "attested archive changed")
named_bundle = step(build_job, "Name provenance bundle")
check(named_bundle.fetch("env").fetch("PROVENANCE_BUNDLE") == '${{ steps.provenance.outputs.bundle-path }}', "attestation bundle output changed")
check(named_bundle.fetch("run").include?('cp "$PROVENANCE_BUNDLE" "$ARCHIVE.provenance.sigstore.json"'), "provenance bundle is not copied to its published name")
check(named_bundle.fetch("run").include?('cygpath -u "$PROVENANCE_BUNDLE"'), "Windows provenance path is not normalized for bash")
check(step(build_job, "Require all release assets").fetch("run").include?('test -s "$asset"'), "partial uploads may hide a missing archive asset")
check(build.fetch("permissions") == {}, "reusable workflow grants default permissions")

Dir.mktmpdir("prim-release-sign-") do |directory|
  recorder = File.join(directory, "cosign")
  arguments = File.join(directory, "arguments")
  File.write(recorder, "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$COSIGN_ARGUMENTS\"\n")
  File.chmod(0755, recorder)
  _, status = run_bash(sign, {
    "PATH" => "#{directory}:#{ENV.fetch('PATH')}",
    "COSIGN_ARGUMENTS" => arguments,
    "ARCHIVE" => "prim-test.tar.gz"
  }, directory)
  check(status.success?, "archive signing command failed under recorder")
  check(File.readlines(arguments, chomp: true) == %w[sign-blob --yes --bundle prim-test.tar.gz.sigstore.json prim-test.tar.gz], "archive signing arguments changed")
end

release = workflow(".github/workflows/release.yml")
jobs = release.fetch("jobs")
matrix = jobs.fetch("build").fetch("strategy").fetch("matrix").fetch("include")
targets = matrix.map { |entry| entry.fetch("target") }
check(
  targets == %w[
    x86_64-unknown-linux-musl
    aarch64-unknown-linux-musl
    x86_64-apple-darwin
    aarch64-apple-darwin
    x86_64-pc-windows-msvc
  ],
  "release target matrix changed"
)
check(jobs.fetch("build").fetch("uses") == "./.github/workflows/release-build.yml", "matrix does not call reusable build")
check(jobs.fetch("build").fetch("with") == {
  "target" => '${{ matrix.target }}', "runner" => '${{ matrix.os }}',
  "use_cross" => '${{ matrix.use_cross }}', "extension" => '${{ matrix.ext }}'
}, "caller input wiring changed")
check(matrix.map { |entry| entry.fetch("use_cross") } == [false, true, false, false, false], "cross target changed")
check(matrix.map { |entry| entry.fetch("ext") } == ["", "", "", "", ".exe"], "binary extension mapping changed")
check(
  jobs.fetch("build").fetch("permissions") == {
    "contents" => "read",
    "id-token" => "write",
    "attestations" => "write",
    "artifact-metadata" => "write"
  },
  "caller build permissions changed"
)

sbom = jobs.fetch("sbom")
check(sbom.fetch("permissions") == {"contents" => "read", "id-token" => "write"}, "SBOM permissions changed")
sbom_uses = sbom.fetch("steps").map { |candidate| candidate["uses"] }.compact
check(sbom_uses.include?("anchore/sbom-action@v0.24.2"), "SBOM action pin changed")
check(sbom_uses.include?("sigstore/cosign-installer@v4.1.2"), "SBOM cosign installer pin changed")
generate = step(sbom, "Generate SPDX SBOM").fetch("with")
check(generate["format"] == "spdx-json", "SBOM format is not SPDX JSON")
check(generate["syft-version"] == "v1.51.1", "Syft pin changed")
check(generate["upload-artifact"] == false, "SBOM action uploads behind the verifier")
check(generate["upload-release-assets"] == false, "SBOM action publishes behind the verifier")
check(generate["path"] == ".", "SBOM is not generated from the release checkout")
check(sbom.fetch("steps").first.fetch("with").fetch("ref") == '${{ github.sha }}', "SBOM checkout is not pinned to the tagged event commit")
check(step(sbom, "Validate SPDX version").fetch("run").include?('spdxVersion == "SPDX-2.3"'), "SPDX-2.3 is not asserted")
check(step(sbom, "Sign SBOM").fetch("run").include?("cosign sign-blob --yes --bundle"), "SBOM is not signed")
sbom_upload = step(sbom, "Upload SBOM assets").fetch("with")
check(sbom_upload.fetch("if-no-files-found") == "error", "missing SBOM upload may pass")
check(sbom_upload.fetch("path").lines.map(&:strip) == ['${{ steps.metadata.outputs.sbom }}', '${{ steps.metadata.outputs.sbom }}.sigstore.json'], "SBOM upload does not contain exactly two assets")
check(step(sbom, "Require SBOM assets").fetch("run").include?('test -s "$SBOM.sigstore.json"'), "partial SBOM upload may pass")

Dir.mktmpdir("prim-sbom-sign-") do |directory|
  recorder = File.join(directory, "cosign")
  arguments = File.join(directory, "arguments")
  File.write(recorder, "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$COSIGN_ARGUMENTS\"\n")
  File.chmod(0755, recorder)
  _, status = run_bash(step(sbom, "Sign SBOM").fetch("run"), {
    "PATH" => "#{directory}:#{ENV.fetch('PATH')}",
    "COSIGN_ARGUMENTS" => arguments,
    "SBOM" => "prim-0.9.0.spdx.json"
  }, directory)
  check(status.success?, "SBOM signing command failed under recorder")
  check(File.readlines(arguments, chomp: true) == %w[sign-blob --yes --bundle prim-0.9.0.spdx.json.sigstore.json prim-0.9.0.spdx.json], "SBOM signing arguments changed")
end

verify = jobs.fetch("verify")
check(verify.fetch("needs") == %w[build sbom], "verifier dependencies changed")
check(verify.fetch("permissions") == {"contents" => "read", "attestations" => "read"}, "verifier permissions changed")
verification = step(verify, "Verify release contract").fetch("run")
[
  "sha256sum -c",
  "cosign verify-blob",
  "--certificate-identity",
  "--certificate-oidc-issuer https://token.actions.githubusercontent.com",
  "gh attestation verify",
  "--repo \"$GITHUB_REPOSITORY\"",
  "--signer-workflow driftsys/prim/.github/workflows/release-build.yml",
  '--bundle "$archive.provenance.sigstore.json"',
  '--source-ref "$GITHUB_REF" --source-digest "$release_commit"',
  '--signer-digest "$release_commit"',
  "--deny-self-hosted-runners",
  '.verificationResult.statement.subject == [{name: $name, digest: {sha256: $digest}}]',
  'build_identity="https://github.com/$GITHUB_REPOSITORY/.github/workflows/release-build.yml@$GITHUB_REF"',
  'sbom_identity="https://github.com/$GITHUB_REPOSITORY/.github/workflows/release.yml@$GITHUB_REF"',
  '--certificate-identity "$sbom_identity"',
  "spdxVersion",
  "git merge-base --is-ancestor",
  'test "$release_commit" = "$GITHUB_SHA"',
  "expected-assets.txt"
].each do |fragment|
  check(verification.include?(fragment), "verifier is missing #{fragment.inspect}")
end
download = step(verify, "Download all artifacts").fetch("with")
check(download == {"path" => "incoming", "pattern" => "prim-*"}, "producer artifacts must remain separate and exclude rerun verifier outputs")
check(verification.include?("expected-artifacts.txt"), "verifier does not pin producer artifact names")
check(verification.include?('source="incoming/prim-$target"'), "verifier does not inspect target artifacts separately")
check(verification.include?("source=incoming/prim-sbom"), "verifier does not inspect the SBOM artifact separately")
verified_upload = step(verify, "Upload verified release assets").fetch("with")
check(verified_upload == {"name" => 'verified-release-assets-${{ github.run_attempt }}', "path" => "dist/*", "if-no-files-found" => "error"}, "verified artifact handoff changed")
check(step(verify, "Upload verified release assets").fetch("id") == "verified", "verified artifact output source changed")
check(verify.fetch("outputs") == {"artifact_id" => '${{ steps.verified.outputs.artifact-id }}'}, "verified artifact ID is not exported")
check(!verification.include?("--cert-identity "), "gh signer-workflow and cert-identity are mutually exclusive")
check(release.fetch("permissions") == {}, "release workflow grants default permissions")
check(jobs.fetch("release").fetch("permissions") == {"contents" => "write"}, "release permissions changed")
check(jobs.fetch("publish").fetch("permissions") == {"contents" => "read"}, "crates.io job permissions changed")

%w[release publish].each do |job_name|
  publication = jobs.fetch(job_name)
  check(publication.fetch("needs") == "verify", "#{job_name} bypasses verifier")
  check(!publication.key?("if"), "#{job_name} overrides successful dependency gating")
  check(!publication.key?("continue-on-error"), "#{job_name} tolerates publication failure")
  check(publication.fetch("steps").none? { |candidate| candidate.key?("continue-on-error") }, "#{job_name} contains a failure-tolerant step")
end
check(!verify.key?("continue-on-error"), "release verification may fail open")
check(verify.fetch("steps").none? { |candidate| candidate.key?("continue-on-error") }, "release verification contains a failure-tolerant step")
release_steps = jobs.fetch("release").fetch("steps")
check(release_steps.none? { |candidate| candidate.key?("run") }, "release job transforms artifacts after verification")
check(step(jobs.fetch("release"), "Download verified artifacts").fetch("with") == {"artifact-ids" => '${{ needs.verify.outputs.artifact_id }}', "path" => "dist", "merge-multiple" => true}, "publication bypasses immutable verified asset handoff")
check(step(jobs.fetch("release"), "Create release").fetch("with").fetch("files") == "dist/*\n", "release asset glob changed")
check(step(jobs.fetch("publish"), "Publish crates").fetch("run").include?("cargo publish"), "crates.io publication disappeared")
check(step(jobs.fetch("publish"), "Publish crates").fetch("run").include?('if [ "$pkg_version" = "$published" ]; then'), "crates.io publication is not idempotent")
check(jobs.fetch("publish").fetch("steps").none? { |candidate| candidate.fetch("uses", "").start_with?("actions/download-artifact@") }, "crates.io job downloads release archives")
[build_job, sbom, verify, jobs.fetch("publish")].each do |job|
  checkout = job.fetch("steps").find { |candidate| candidate["uses"] == "actions/checkout@v7" }
  check(checkout.fetch("with").fetch("ref") == '${{ github.sha }}', "release source checkout is mutable")
  check(checkout.fetch("with").fetch("persist-credentials") == false, "checkout retains a repository credential")
end

Dir.mktmpdir("prim-publish-idempotency-") do |directory|
  calls = File.join(directory, "publish-calls")
  cargo = <<~SH
    #!/bin/sh
    if [ "$1" = metadata ]; then
      printf '%s\n' '{"packages":[{"name":"prim-fmt","version":"0.9.0"},{"name":"prim-cli","version":"0.9.0"}]}'
    elif [ "$1" = publish ]; then
      printf '%s\n' "$*" >> "$PUBLISH_CALLS"
    else
      exit 99
    fi
  SH
  curl = <<~SH
    #!/bin/sh
    printf '%s\n' '{"crate":{"newest_version":"0.9.0"}}'
  SH
  {"cargo" => cargo, "curl" => curl, "sleep" => "#!/bin/sh\nexit 0\n"}.each do |name, source|
    path = File.join(directory, name)
    File.write(path, source)
    File.chmod(0755, path)
  end
  output, status = run_bash(step(jobs.fetch("publish"), "Publish crates").fetch("run"), {
    "PATH" => "#{directory}:#{ENV.fetch('PATH')}",
    "PUBLISH_CALLS" => calls,
    "CARGO_REGISTRY_TOKEN" => "unused-test-token"
  }, directory)
  check(status.success?, "idempotent crates.io path failed:\n#{output}")
  check(!File.exist?(calls) || File.zero?(calls), "already-published crates were published again")
end

ci = workflow(".github/workflows/ci.yml")
ci_jobs = ci.fetch("jobs")
check(ci.fetch(true).keys.sort == %w[pull_request push schedule], "cargo audit CI triggers changed")
audit = ci_jobs.fetch("audit")
check(audit.fetch("steps").any? { |candidate| candidate["uses"] == "rustsec/audit-check@v2" }, "cargo audit action disappeared")
check(!audit.key?("if") && !audit.key?("continue-on-error"), "cargo audit may skip or fail open")
check(audit.fetch("steps").none? { |candidate| candidate.key?("continue-on-error") }, "cargo audit step may fail open")
check(ci_jobs.fetch("ci").fetch("needs").include?("audit"), "cargo audit no longer gates CI")
release_contract = ci_jobs.fetch("release-contract-test")
check(release_contract.fetch("steps").any? { |candidate| candidate["run"] == "bash tools/bash_unit spec/install/release_contract_test.sh" }, "release contract CI job depends on an unavailable command")

# Execute the actual gate with controlled trust-service responses. These tests
# exercise rejection and argument wiring, not hosted OIDC or cryptographic trust.
trust_stub = <<~'RUBY'
  #!/usr/bin/env ruby
  require "digest"
  require "json"
  command = File.basename($PROGRAM_NAME)
  value = ->(option) { ARGV.fetch(ARGV.index(option) + 1) }
  commit = "a" * 40
  case command
  when "git"
    case ARGV.first
    when "rev-parse"
      abort "wrong ref lookup" unless [%w[rev-parse HEAD], ["rev-parse", "refs/tags/v0.9.0^{commit}"]].include?(ARGV)
      if ARGV.last == "HEAD"
        puts ENV.fetch("HEAD_COMMIT", commit)
      else
        puts ENV.fetch("TAG_COMMIT", commit)
      end
    when "fetch"
      abort "wrong fetch operands" unless ARGV == ["fetch", "origin", "main:refs/remotes/origin/main"]
      exit 0
    when "merge-base"
      abort "wrong ancestry operands" unless ARGV == ["merge-base", "--is-ancestor", commit, "origin/main"]
      exit(ENV["REJECT_ANCESTRY"] == "1" ? 1 : 0)
    else abort "unexpected git operation"
    end
  when "cosign"
    artifact = ARGV.last
    is_sbom = artifact.end_with?(".spdx.json")
    workflow = is_sbom ? "release.yml" : "release-build.yml"
    identity = "https://github.com/driftsys/prim/.github/workflows/#{workflow}@refs/tags/v0.9.0"
    abort "wrong cosign identity" unless value.call("--certificate-identity") == identity
    abort "wrong cosign issuer" unless value.call("--certificate-oidc-issuer") == "https://token.actions.githubusercontent.com"
    abort "wrong signature bundle" unless value.call("--bundle") == "#{artifact}.sigstore.json"
    abort "rejected signature" if is_sbom && ENV["REJECT_SBOM"] == "1"
    abort "rejected signature" if !is_sbom && ENV["REJECT_SIGNATURE"] == File.basename(artifact)
    File.open(ENV.fetch("COSIGN_CALLS"), "a") { |file| file.puts File.basename(artifact) }
  when "gh"
    artifact = ARGV.fetch(2)
    abort "wrong gh operation" unless ARGV[0, 2] == %w[attestation verify]
    {
      "--repo" => "driftsys/prim",
      "--bundle" => "#{artifact}.provenance.sigstore.json",
      "--signer-workflow" => "driftsys/prim/.github/workflows/release-build.yml",
      "--source-ref" => "refs/tags/v0.9.0",
      "--source-digest" => commit,
      "--signer-digest" => commit,
      "--cert-oidc-issuer" => "https://token.actions.githubusercontent.com",
      "--format" => "json"
    }.each { |option, expected| abort "wrong #{option}" unless value.call(option) == expected }
    abort "allows self-hosted runner" unless ARGV.include?("--deny-self-hosted-runners")
    abort "incompatible identity flags" if ARGV.include?("--cert-identity")
    abort "rejected provenance" if ENV["REJECT_PROVENANCE"] == File.basename(artifact)
    File.open(ENV.fetch("GH_CALLS"), "a") { |file| file.puts File.basename(artifact) }
    subject = {name: artifact, digest: {sha256: Digest::SHA256.file(artifact).hexdigest}}
    subject[:name] = "other.tar.gz" if ENV["BAD_SUBJECT"] == "name"
    subject[:digest][:sha256] = "0" * 64 if ENV["BAD_SUBJECT"] == "digest"
    statement = {predicateType: "https://slsa.dev/provenance/v1", subject: [subject]}
    statement[:subject] = [] if ENV["BAD_SUBJECT"] == "empty"
    statement[:predicateType] = "https://example.com/untrusted" if ENV["BAD_SUBJECT"] == "predicate"
    results = [{verificationResult: {statement: statement}}]
    results = [] if ENV["BAD_SUBJECT"] == "no results"
    puts JSON.generate(results)
  else
    abort "unexpected trust command"
  end
RUBY

cases = {
  "valid" => {},
  "missing asset" => {},
  "extra asset" => {},
  "extra producer artifact" => {},
  "cross-artifact duplicate" => {},
  "wrong SPDX version" => {},
  "wrong provenance subject name" => {"BAD_SUBJECT" => "name"},
  "wrong provenance subject digest" => {"BAD_SUBJECT" => "digest"},
  "empty provenance subjects" => {"BAD_SUBJECT" => "empty"},
  "wrong provenance predicate" => {"BAD_SUBJECT" => "predicate"},
  "no verified attestations" => {"BAD_SUBJECT" => "no results"},
  "SBOM signature rejection" => {"REJECT_SBOM" => "1"},
  "tag outside main" => {"REJECT_ANCESTRY" => "1"},
  "moved tag" => {"GITHUB_SHA" => "b" * 40},
  "checkout mismatch" => {"HEAD_COMMIT" => "b" * 40}
}
targets.each do |target|
  archive = "prim-#{target}.tar.gz"
  cases["tampered archive #{target}"] = {"TAMPER_TARGET" => target}
  cases["wrong checksum subject #{target}"] = {"BAD_CHECKSUM_TARGET" => target}
  cases["signature rejection #{target}"] = {"REJECT_SIGNATURE" => archive}
  cases["provenance rejection #{target}"] = {"REJECT_PROVENANCE" => archive}
end
cases.each do |name, failure_env|
  Dir.mktmpdir("prim-release-verifier-") do |directory|
    incoming = File.join(directory, "incoming")
    bin = File.join(directory, "bin")
    runner = File.join(directory, "runner")
    cosign_calls = File.join(directory, "cosign-calls")
    gh_calls = File.join(directory, "gh-calls")
    FileUtils.mkdir_p([incoming, bin, runner])
    %w[git cosign gh].each do |command|
      path = File.join(bin, command)
      File.write(path, trust_stub)
      File.chmod(0755, path)
    end
    # shasum is also available on macOS, where GNU sha256sum may be absent.
    File.write(File.join(bin, "sha256sum"), "#!/bin/sh\nexec shasum -a 256 \"$@\"\n")
    File.chmod(0755, File.join(bin, "sha256sum"))
    targets.each do |target|
      archive = "prim-#{target}.tar.gz"
      contents = "release bytes for #{target}\n"
      producer = File.join(incoming, "prim-#{target}")
      FileUtils.mkdir_p(producer)
      File.write(File.join(producer, archive), contents)
      File.write(File.join(producer, "#{archive}.sha256"), "#{Digest::SHA256.hexdigest(contents)}  #{archive}\n")
      File.write(File.join(producer, "#{archive}.sigstore.json"), "{}\n")
      File.write(File.join(producer, "#{archive}.provenance.sigstore.json"), "{}\n")
    end
    sbom_producer = File.join(incoming, "prim-sbom")
    FileUtils.mkdir_p(sbom_producer)
    sbom_path = File.join(sbom_producer, "prim-0.9.0.spdx.json")
    File.write(sbom_path, JSON.generate(spdxVersion: "SPDX-2.3"))
    File.write("#{sbom_path}.sigstore.json", "{}\n")
    first_producer = File.join(incoming, "prim-#{targets.first}")
    archive = File.join(first_producer, "prim-#{targets.first}.tar.gz")
    case name
    when "missing asset" then File.delete("#{archive}.sigstore.json")
    when "extra asset" then File.write(File.join(first_producer, "unexpected.txt"), "unexpected")
    when "extra producer artifact" then FileUtils.mkdir_p(File.join(incoming, "unexpected-producer"))
    when "cross-artifact duplicate"
      File.write(File.join(first_producer, "prim-#{targets[1]}.tar.gz"), "duplicate")
    when "wrong SPDX version" then File.write(sbom_path, JSON.generate(spdxVersion: "SPDX-2.2"))
    end
    if failure_env["TAMPER_TARGET"]
      target = failure_env["TAMPER_TARGET"]
      tampered = File.join(incoming, "prim-#{target}", "prim-#{target}.tar.gz")
      File.write(tampered, "changed bytes")
    end
    if failure_env["BAD_CHECKSUM_TARGET"]
      target = failure_env["BAD_CHECKSUM_TARGET"]
      bad_archive = "prim-#{target}.tar.gz"
      checksum = File.join(incoming, "prim-#{target}", "#{bad_archive}.sha256")
      other_target = (targets - [target]).first
      other_archive = "prim-#{other_target}.tar.gz"
      other_path = File.join(incoming, "prim-#{other_target}", other_archive)
      File.write(checksum, "#{Digest::SHA256.file(other_path).hexdigest}  #{other_archive}\n")
    end
    env = {
      "PATH" => "#{bin}:#{ENV.fetch('PATH')}", "RUNNER_TEMP" => runner,
      "GITHUB_REPOSITORY" => "driftsys/prim", "GITHUB_REF" => "refs/tags/v0.9.0",
      "GITHUB_REF_NAME" => "v0.9.0", "GITHUB_SHA" => "a" * 40,
      "COSIGN_CALLS" => cosign_calls, "GH_CALLS" => gh_calls
    }.merge(failure_env)
    output, status = Open3.capture2e(env, "bash", "-c", verification, chdir: directory)
    check(status.success? == (name == "valid"), "verifier case #{name.inspect} returned #{status.exitstatus}:\n#{output}")
    if name == "valid"
      expected_archives = targets.map { |target| "prim-#{target}.tar.gz" }
      check(File.readlines(gh_calls, chomp: true) == expected_archives, "provenance verification did not cover every archive")
      check(File.readlines(cosign_calls, chomp: true) == expected_archives + ["prim-0.9.0.spdx.json"], "signature verification did not cover every archive and the SBOM")
    end
  end
end
