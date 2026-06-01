set shell := ["bash", "-euo", "pipefail", "-c"]

app_name := "soyel"
app_bundle := "src-tauri/target/release/bundle/macos/" + app_name + ".app"
install_dir := "/Applications"
installed_app := install_dir + "/" + app_name + ".app"

default:
    @just --list

check:
    bun run check

build:
    env CARGO_BUILD_RUSTC_WRAPPER= bun tauri build --bundles app

install:
    @if pgrep -x "{{app_name}}" >/dev/null; then \
        echo "{{app_name}} is running. Quit it before installing."; \
        exit 1; \
    fi
    env CARGO_BUILD_RUSTC_WRAPPER= bun tauri build --bundles app
    @test -d "{{app_bundle}}"
    rm -rf "{{installed_app}}"
    cp -R "{{app_bundle}}" "{{install_dir}}/"
    @echo "Installed {{installed_app}}"

