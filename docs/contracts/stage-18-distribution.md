# Stage 18 Contract: Distribution and Release

**Issue:** [#240](https://github.com/SWOFART/open-circuit/issues/240). **Parent:** [#163](https://github.com/SWOFART/open-circuit/issues/163). **Status:** proposal for the approval of both workstreams and the project owner. Implementation (#241 to #244) follows acceptance of this contract. Several decisions cannot be made by engineering alone (a certificate, a key, a privacy default); they are listed in section 12.

**Scope:** How Open Circuit reaches a user on **Windows**: the installer, how builds are signed and where the secrets live, how an installed application learns of and applies an update, how a version is numbered, how release notes are made and how a bad release is withdrawn. It changes no simulation, editor or file-format behaviour.

**Primary sources.** The Tauri 2 documentation as read on 2026-11-26: [Windows installer](https://v2.tauri.app/distribute/windows-installer/), [Updater plugin](https://v2.tauri.app/plugin/updater/), [Windows code signing](https://v2.tauri.app/distribute/sign/windows/) and the [configuration reference](https://v2.tauri.app/reference/config/). Where this contract relies on a statement of those pages it says so; anything the pages do not state is marked **to be confirmed in #241 or #242** rather than assumed.

## 1. Boundary and today's state

- Today CI builds a bare executable on three platforms: `crates/app/tauri.conf.json` has `bundle.active` set to `false`, so `pnpm build` runs `tauri build` and outputs the executable only. No installer, signature, updater or tagged release exists.
- The version `0.1.0` is written in four places: `Cargo.toml` (workspace), `package.json`, `crates/app/package.json` and `crates/app/tauri.conf.json`.
- The licence is GPL-3.0-or-later. The CI workflow pins actions by commit hash and runs with read-only permissions; it does not run the test suites (since #296).
- **Stage 18 is Windows only.** macOS (a notarized app and disk image) and Linux (packages) are deferred, each with its own acceptance record when reached (ARCHITECTURE.md, "Platforms"). The pull-request CI keeps its three-platform build unchanged.

## 2. The installer

- **Format: NSIS `-setup.exe`**, built on a Windows runner (`bundle.targets` `["nsis"]`). The Tauri documentation offers NSIS and the WiX-based MSI and recommends neither; the reasons for NSIS here are that its **default install is per-user** under `%LOCALAPPDATA%` with no administrator rights (`bundle.windows.nsis.installMode` left at its default), which is what an unattended update needs, and that it needs no WiX toolchain. **MSI is deferred**: it is the format an administrator wants for a managed, per-machine deployment, and it can be added later without changing anything else here.
- **Installed as** `Open Circuit` with the identifier `org.swofart.opencircuit`, for the current user, with a Start menu entry and an uninstaller in the Windows list of apps. The installer carries the licence (`LICENSE`) and the licences of bundled third-party material (today the DejaVu font licence, already a bundled resource); the tool that gathers the notices of Rust and JavaScript dependencies is chosen in #243.
- **Uninstall removes the program and leaves the user's files.** Projects and assignments live where the user saved them, never in the install directory. The application's own data (the recovery file of stage 16, the web view's profile with its stored preferences) lives under the per-user application data directory; the uninstaller should offer to delete it, **unchecked by default** (whether the NSIS installer of Tauri provides this option out of the box is **to be confirmed in #241**; the pages read for this contract do not say).
- **WebView2.** Windows 10 (April 2018 or later) and Windows 11 ship the runtime (the installer page of the Tauri documentation says so). The installer uses `bundle.windows.webviewInstallMode` `downloadBootstrapper` (the documented default: no added size, needs the network at install time). An **offline installer** variant (`offlineInstaller`, about 127 MB added, for machines with no network, such as a classroom) is deferred and is decision 3 of section 12.
- **Configuration.** The committed `tauri.conf.json` keeps `bundle.active` `false`, so the pull-request builds do not start producing installers on macOS and Linux. The release workflow adds the bundle settings with a second file, `crates/app/tauri.release.conf.json`, passed with `--config` (the documented way to merge a configuration file on the command line). That file also holds `bundle.createUpdaterArtifacts` (section 5) and the updater's public key; it contains no secret.

## 3. Versions and tags

- **Semantic Versioning.** The version is `MAJOR.MINOR.PATCH` with optional pre-release identifiers (`0.3.0-rc.1`). While the major version is 0, a minor increment may change behaviour or the file format (with the migrations the format contracts already require) and a patch increment only fixes defects.
- **One source of truth:** the workspace `Cargo.toml`. A script, `scripts/check-version.mjs`, run by CI on every pull request, fails when the other three places differ from it. Releasing means changing the version in one commit through the usual reviewed pull request.
- **A release is a tag** `vMAJOR.MINOR.PATCH` on a commit of `main` whose version is exactly that. The release workflow refuses a tag whose version does not match the files or whose commit is not on `main`. Tags matching `v*` are protected: only the project owner and the maintainers they name can create them, and they cannot be moved or deleted once a release uses them.
- **Stable and pre-release.** A tag with a pre-release identifier makes a GitHub **pre-release**, which the updater never offers (section 6). Only a version without an identifier is stable. A pre-release tag runs the same release workflow in the `release` environment, because it is a published object; it **builds no updater artifact** (`createUpdaterArtifacts` is on only for a tag without an identifier), so the update key is used only for stable releases, and its installer is signed or, until decision 1 is settled, labelled an unsigned preview.
- **The format version is separate.** The project schema (3 today) and the assignment version are not the application version; a release note says when a release changes either.

## 4. The release pipeline

A new workflow, `.github/workflows/release.yml` (#241), separate from `ci.yml`:

- **Triggers:** a pushed tag `v*` (a real release) and `workflow_dispatch` (a **dry run**: the same build, no signing, no release object, the installer kept as a workflow artifact for 7 days). The dry run does **not** use the `release` environment and so has no secret: it builds with a variant of the release configuration **without `bundle.createUpdaterArtifacts`** (that setting needs the update private key at build time, section 5.2), skips steps 3, 4 and 6 below, and runs the smoke test of step 5 on the unsigned installer.
- **Order:** (1) *verify*: the formatting, lint, type, bundle and boundary checks of CI **and the full test suites**, because the pull-request CI does not run them; (2) *build* on `windows-latest` with `pnpm install --frozen-lockfile`, `cargo build --locked` through `tauri build`, the pinned toolchain of `rust-toolchain.toml`, and a final check that the lock files are unchanged; (3) *sign* (section 5); (4) *verify the signature* of the installer and of the update artifact, failing on any problem; (5) *smoke test* on a fresh runner job: silent install, start the installed application and check that it runs and answers a self-test, uninstall, and check that the program files are gone and a file the test placed in a user directory is not; (6) *draft* a GitHub Release with the installer, its update signature (`.sig`), `latest.json` (section 6), a file of SHA-256 checksums and the release notes.
- **A person publishes.** The workflow creates a **draft** release. Publishing it (which is what makes the updater offer it) is a deliberate act of the project owner after the checklist of section 9; neither a workflow nor an agent publishes.
- **Secrets.** Signing material lives only in a GitHub **environment** named `release` with required reviewers, available to the release workflow on a protected tag and never to a pull request, a fork or the dry run. Permissions are the least the job needs (`contents: write` for the draft, `id-token: write` only if signing uses OIDC). No step prints a secret, a command line with one, or an environment dump; the workflow never uses `pull_request_target`; build artifacts contain no key, certificate or token. A leak check (a search of the logs and artifacts for the secret values' shapes) is part of #241's tests.
- **Reproducibility** here means a **documented, repeatable procedure from a tag with pinned inputs** (toolchain, lock files, action hashes), not bit-identical installers: an NSIS installer and a signature carry timestamps. Every release records the SHA-256 of what it publishes.

## 5. Signing

There are **two independent signatures**, for two different purposes, with two different keys.

### 5.1 Windows code signing (Authenticode)

- **Why.** An unsigned installer downloaded in a browser meets a SmartScreen warning, and on Windows 11 with Smart App Control an unknown unsigned application can be blocked outright. Signing prevents the first. The Tauri documentation also notes that since 2024 an EV certificate no longer gives an immediate reputation: **a newly signed release can still show a warning with any certificate**, and reputation grows with signing every release with the same identity. Signing is necessary and not sufficient.
- **Where the key lives: never in the repository, never in an artifact, never on a developer's disk.** The preferred arrangement is a **cloud signing service** whose key never leaves it and which a workflow calls with a short-lived identity: **Azure Artifact Signing** (the documented `signCommand` with `artifact-signing-cli`, previously "Trusted Signing"), authenticated by **OIDC federation** so that no long-lived secret exists. The alternatives the documentation lists are a certificate in **Azure Key Vault** (with the `relic` tool) and an OV or EV certificate as a `.pfx` held as an environment secret (the documentation's own GitHub example, which it describes only for older OV certificates and which keeps a reusable private key in a secret, the weakest of the three). The choice depends on an identity validation and a contract the project owner must obtain, and on eligibility rules this contract does not assume (decision 1).
- **Configuration** is `bundle.windows.signCommand` in the release configuration file, with `%1` as the path of the file to sign, a timestamp server so that signatures outlive the certificate, and the `sha256` digest. The exact command, the account and the profile names are not secrets and are committed; the credentials are not.
- **Until a certificate exists** the pipeline can run, produce an **unsigned installer** and publish it only as a **pre-release labelled "unsigned preview"** whose notes say what the user will see. An unsigned build is never offered by the updater and never marked stable (section 6).
- **Verification in CI:** `Get-AuthenticodeSignature` (or `signtool verify /pa`) on the installer and on the installed executables; the step fails unless the status is `Valid`, the signer is the expected subject and a timestamp is present.

### 5.2 The update signature (Tauri minisign)

- **Why.** The updater plugin verifies every update against a public key embedded in the application; per the documentation this check **cannot be disabled**. It protects against a tampered download and against a hijacked endpoint, which Authenticode alone does not do for an in-app update.
- **The keypair** is generated once, offline, by the project owner with `tauri signer generate`. The **public key** is committed in `plugins.updater.pubkey` (the documentation requires the key's content, not a path). The **private key and its password** are stored only as the secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` of the `release` environment (build-time environment variables; `.env` files do not work for them), and in **two offline backups** the owner keeps (a password manager and a sealed copy). **If the key is lost, no installed copy can ever be updated again** (the documentation says so), so the backups are part of the release checklist.
- **Rotation.** A key can be replaced by publishing one release signed with the old key whose application embeds the new public key; every later release uses the new key. A compromised key cannot be handled that way if the attacker can sign too; the response is to publish a fresh installer through the website and the release page and to tell users, which is why the installer is also always published directly.
- `bundle.createUpdaterArtifacts` is `true` (the Tauri 2 format: the installer itself plus its `.sig` file); `"v1Compatible"` is not used.

## 6. Updates

- **Mechanism:** the Tauri **updater plugin**, run from `crates/app` (Rust), so the **web view gains no network access** and the content security policy of `tauri.conf.json` is unchanged. The interface can only ask the application to check for an update and to install the one it found; it cannot name a URL or a file.
- **Endpoint:** one static file, `latest.json`, attached to every **stable** GitHub Release, read from `https://github.com/SWOFART/open-circuit/releases/latest/download/latest.json` (the documentation lists this form). GitHub's "latest" alias skips drafts and pre-releases, which is what gives a stable channel. The file holds `version`, `pub_date`, `notes` and, for `windows-x86_64`, the `url` of the installer and the **content** of its `.sig` file (not a path). TLS is enforced in production and `dangerousInsecureTransportProtocol` is never set. There is **one channel**; a pre-release channel is deferred.
- **Behaviour.**
  - The application checks **once at start and then at most once a day while it is open**, and the user can choose **Check for updates** at any time. A check sends only what the endpoint URL carries (the current version, `windows` and the architecture) and exposes the user's IP address to GitHub as any download does; **no identifier, no usage data and no telemetry** is sent.
  - When a newer version exists the user sees its version and notes and chooses **Install** or **Later**. **Nothing is installed without that choice**, and nothing is downloaded in a way that interrupts work.
  - Installing first applies the rules of the unsaved-changes dialog (the user saves, discards or cancels; with stage 16's autosave a recovery file is also written), because the documentation states that on Windows the application **exits automatically** when the installer runs. The install mode is `passive` (a small progress window, no interaction; the documented default).
  - The user can turn the automatic check off in the application's preferences (the web view's local storage, as the stage 11 and stage 16 preferences). The default is decision 4.
- **Failure paths** each leave the installed application exactly as it was and show a plain message: no network or an unreachable endpoint (silent for the automatic check, a message for a manual one); a response that is not valid JSON or lacks a required field (the plugin validates the whole file before it compares versions, per the documentation); a version that is not newer; a **signature that does not verify** (wrong key, altered installer, altered `.sig`); a truncated or failed download; an installer that fails to start. These are tested in #242, with a local test server.
- **Version comparison:** the plugin's default, an update is applied only when its version is **greater** than the running one. Downgrades are not enabled (section 7).

## 7. Rollback and withdrawal

- **The updater never moves backward** and this stage does not enable downgrades (the plugin supports them only through a custom version comparator, which would also let a hostile endpoint downgrade an installation).
- **A bad release is handled in this order.** (1) **Stop new installs:** un-publish or delete the release, or mark it a pre-release, so that GitHub's "latest" points at the previous stable release and the updater stops offering the bad one; a person who has not updated stays on the good version. (2) **Roll forward:** publish a new patch version that contains the fix or a revert; installed copies of the bad release update to it normally. (3) **Direct users on the bad version** to install the previous installer from the release page by hand if they cannot wait; installing an older version over a newer one keeps their files, because the files and the application data are never in the install directory, and a project saved by a newer format version is refused by an older application with the message the format contracts already define (no downgrade of data).
- **Every release keeps its installer on the release page** and its checksums, so a previous version is always available.
- A release record (section 9) states, for each release, the version it follows and whether a project or assignment format changed, so that a rollback decision knows what a downgrade would refuse.

## 8. Release notes and the changelog

- `CHANGELOG.md` at the root, in the *Keep a Changelog* layout, with a section for each released version and an *Unreleased* section.
- A script (#243) drafts the section from the **merged pull requests since the previous tag**, grouped by the type prefix of their titles (`feat`, `fix`, `docs`, `test`, `ci`), each line with its pull request link; a person edits the draft into prose before the tag. The same text, with the list of the stages whose acceptance record the release contains, is the body of the GitHub Release and the `notes` field of `latest.json`.
- The notes also state: whether the project or assignment format changed, any removed behaviour, whether the build is signed, and the SHA-256 of the installer.

## 9. The release checklist and the dry run

A checklist in `docs/release.md` (#243), completed for each release in the release pull request or issue:

1. The version, in the four places, and the changelog section are merged; `scripts/check-version.mjs` passes.
2. Every stage the release claims has its **acceptance record** in `docs/acceptance/`, and the record's pending items are either closed or named in the notes.
3. CI is green on the three platforms for the release commit, and the release workflow's own full test run passed.
4. The dry run (`workflow_dispatch`) of the same commit passed and its installer was installed, run and uninstalled by hand on a clean Windows machine (the update path cannot be tried on a dry run, which builds no updater artifact; it is tried after publishing, item 7).
5. The signing material is in place (decision 1), the updater private key and its backups are confirmed, and the signature checks passed.
6. The release is drafted, its checksums and notes are reviewed, and a **person publishes** it.
7. After publishing: the update path is tried from the previous release on a clean machine, and the result is recorded.

**A dry run is recorded** before the first real release, in `docs/acceptance/stage-18-distribution.md` (#244): the workflow run, the installer's checksum, the clean-machine observations and any problem.

## 10. Security and privacy

- The pipeline's secrets are only in the `release` environment (section 4); no repository file, pull-request run, fork or artifact can read them.
- Dependencies of the pipeline are pinned (actions by hash, as `ci.yml` does); the lock files must be unchanged by the build.
- The application sends nothing to any server except the update check of section 6, to GitHub, with no identifier. This contract adds no telemetry, no crash upload and no account.
- The licence obligations of GPL-3.0-or-later are met by shipping the licence and the notices with the installer and by publishing the source of each released tag (the tag itself).

## 11. Verification and ownership

| Issue | What it delivers | How it is checked |
|---|---|---|
| #241 | `release.yml`, `tauri.release.conf.json`, `scripts/check-version.mjs`; an unsigned NSIS installer from a tag and from a dry run | A tag builds the installer; the silent install, run and uninstall job passes on a clean runner; a search of logs and artifacts finds no secret; the existing CI checks are unchanged |
| #242 | Signing, signature verification, the updater plugin in `crates/app`, `latest.json` generation | The signature checks of 5.1; the failure paths of section 6 against a local server (wrong key, altered file, bad JSON, no newer version, truncated download, unreachable endpoint); the interface offers Install and Later and never installs unasked |
| #243 | `docs/release.md`, `CHANGELOG.md`, the notes script and the dry run | The checklist is followed once for real; the dry run is recorded |
| #244 | `docs/acceptance/stage-18-distribution.md` | A signed installer installs, runs, updates from the previous release and uninstalls on a clean Windows machine, with native observations; CI on the release commit |

Ownership is shared: the Application workstream owns the plugin, the preferences and the messages; the project owner owns the certificate, the update key, the `release` environment and the act of publishing.

## 12. Decisions that need approval

1. **The Windows signing arrangement** (section 5.1): Azure Artifact Signing with OIDC (recommended: no reusable private key in any secret), a certificate in Azure Key Vault, or a `.pfx` secret; and the identity and contract the owner must obtain. Until it exists, only unsigned **pre-releases** are published.
2. **The updater key custody** (section 5.2): that the owner generates and holds the key and keeps two offline backups, and that losing it is accepted as ending updates for installed copies.
3. **NSIS, per-user, with a downloading WebView2 bootstrapper** (section 2); the offline installer and MSI deferred. If classrooms without a network are a target, the offline installer should be built from the first release.
4. **The default of the automatic update check** (section 6): on (recommended, with the data sent stated and a preference to turn it off) or off.
5. **One stable channel** through GitHub Releases and a static `latest.json`; a pre-release channel, delta updates and a self-hosted endpoint deferred.
6. **No downgrades** by the updater and the withdrawal procedure of section 7.
7. **A person publishes** every release; the workflow only drafts.
8. **Tag protection and the `release` environment** with required reviewers (section 4), which only the owner can configure.
9. **The test suites run in the release workflow** even though the pull-request CI skips them.

## 13. Deferred

macOS and Linux builds, notarization, disk images and packages; the MSI installer and per-machine deployment; the offline installer unless decision 3 says otherwise; ARM64; a pre-release update channel; delta updates; the Microsoft Store; telemetry or crash reporting; a self-hosted update server; a software bill of materials; bit-for-bit reproducible installers.
