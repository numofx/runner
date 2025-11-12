# CI/CD Pipeline Documentation

This document describes the Continuous Integration and Continuous Deployment (CI/CD) pipeline for the Numo Engine Arbitrage Bot.

## Overview

The CI/CD pipeline is built using GitHub Actions and consists of multiple workflows that ensure code quality, security, and automated releases.

## Workflows

### 1. CI Workflow (`.github/workflows/ci.yml`)

**Triggers:** Push to main, Pull Requests

This is the main continuous integration workflow that runs on every push and pull request.

#### Jobs:

- **Format Check (`fmt`)**: Verifies code formatting using `cargo fmt`
- **Clippy Lint (`clippy`)**: Runs Rust linter with all warnings as errors
- **Test Suite (`test`)**: Runs all unit and integration tests
- **Build (`build`)**: Builds release binaries for multiple platforms:
  - `x86_64-unknown-linux-gnu` (Linux AMD64)
  - `aarch64-unknown-linux-gnu` (Linux ARM64)
- **Smart Contracts (`contracts`)**: Checks Solidity contracts using Foundry
- **Package Check (`check`)**: Verifies all packages compile correctly
- **CI Success (`ci-success`)**: Aggregates all job results

#### Caching Strategy:
- Cargo registry
- Cargo index
- Build artifacts

### 2. Release Workflow (`.github/workflows/release.yml`)

**Triggers:** Git tags matching `v*.*.*`, Manual dispatch

Automates the release process when a new version is tagged.

#### Jobs:

- **Create Release**: Creates a GitHub release with changelog
- **Build Release Binaries**: Builds optimized binaries for:
  - Linux AMD64
  - Linux ARM64
  - macOS AMD64
  - macOS ARM64
- **Publish to crates.io**: Publishes packages to the Rust package registry

#### Release Assets:
- Compressed binaries (`.tar.gz`)
- SHA256 checksums

### 3. Security Audit Workflow (`.github/workflows/security.yml`)

**Triggers:** Push to main, Pull Requests, Daily schedule (00:00 UTC), Manual dispatch

Performs comprehensive security audits.

#### Jobs:

- **Cargo Audit**: Checks for known security vulnerabilities in dependencies
- **Cargo Deny**: Enforces security and licensing policies
- **Dependency Review**: Reviews dependencies in pull requests
- **Outdated Dependencies**: Checks for outdated crates
- **Slither Analysis**: Analyzes Solidity contracts for vulnerabilities
- **Mythril Analysis**: Additional smart contract security analysis

### 4. Docker Workflow (`.github/workflows/docker.yml`)

**Triggers:** Push to main, Tags, Pull Requests, Manual dispatch

Builds and publishes Docker images to GitHub Container Registry.

#### Features:
- Multi-platform builds (linux/amd64, linux/arm64)
- Image vulnerability scanning with Trivy
- Automatic tagging:
  - Branch name (e.g., `main`)
  - Semantic version (e.g., `v1.0.0`, `1.0`, `1`)
  - Git SHA
  - `latest` for main branch
- Layer caching for faster builds

## Dependabot Configuration

File: `.github/dependabot.yml`

Automated dependency updates:
- **Cargo dependencies**: Weekly updates every Monday
- **GitHub Actions**: Weekly updates every Monday

## GitHub Templates

### Issue Templates

Located in `.github/ISSUE_TEMPLATE/`:

1. **Bug Report** (`bug_report.yml`): Structured bug reporting
2. **Feature Request** (`feature_request.yml`): Feature suggestions
3. **Config** (`config.yml`): Links to discussions and security reporting

### Pull Request Template

File: `.github/PULL_REQUEST_TEMPLATE.md`

Standardized PR format including:
- Description and type of change
- Testing checklist
- Security considerations
- Performance impact

## Security Configuration

### Cargo Deny (`deny.toml`)

Enforces:
- **Advisories**: Denies known vulnerabilities
- **Licenses**: Only allows approved licenses (MIT, Apache-2.0, BSD, etc.)
- **Bans**: Prevents multiple versions and known problematic crates
- **Sources**: Restricts to official crates.io registry

## Docker Configuration

### Dockerfile

Multi-stage build:
1. **Builder stage**: Compiles Rust code
2. **Runtime stage**: Minimal Debian image with only the binary

Features:
- Non-root user execution
- Health checks
- Optimized layer caching

### .dockerignore

Excludes unnecessary files from Docker context to speed up builds.

## Required Secrets

Configure these secrets in GitHub repository settings:

- `GITHUB_TOKEN`: Automatically provided by GitHub Actions
- `CARGO_REGISTRY_TOKEN`: For publishing to crates.io (optional)

## Best Practices

### For Contributors

1. **Before Committing:**
   ```bash
   cargo fmt --all
   cargo clippy --all --all-features
   cargo test --all
   ```

2. **Pull Requests:**
   - Fill out the PR template completely
   - Ensure all CI checks pass
   - Request review from maintainers

### For Maintainers

1. **Releases:**
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```
   The release workflow will automatically:
   - Create a GitHub release
   - Build binaries for all platforms
   - Publish to crates.io (if token is configured)

2. **Security Issues:**
   - Monitor daily security audit reports
   - Update dependencies regularly
   - Use Dependabot PRs to keep dependencies current

## Monitoring

### CI Status
- Check the Actions tab on GitHub for all workflow runs
- Failed workflows will block PR merging (if branch protection is enabled)

### Security
- Daily security audits run automatically
- Review Dependabot PRs weekly
- Monitor security advisory notifications

## Troubleshooting

### Common Issues

1. **Build Failures:**
   - Check that all code is formatted: `cargo fmt --all`
   - Ensure no clippy warnings: `cargo clippy --all`
   - Verify tests pass locally: `cargo test --all`

2. **Docker Build Issues:**
   - Verify Dockerfile syntax
   - Check that all paths in COPY commands exist
   - Ensure .dockerignore doesn't exclude necessary files

3. **Release Workflow Failures:**
   - Verify tag format matches `v*.*.*`
   - Check that CARGO_REGISTRY_TOKEN is set (for crates.io publishing)
   - Ensure Cargo.toml versions match the tag

## Further Information

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Cargo Documentation](https://doc.rust-lang.org/cargo/)
- [Docker Documentation](https://docs.docker.com/)
- [Foundry Documentation](https://book.getfoundry.sh/)
