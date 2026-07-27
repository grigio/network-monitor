{
  description = "Network Monitor - Rust + GTK4 + eBPF connection tracing";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        gtkDeps = with pkgs; [
          gtk4
          libadwaita
          glib
          cairo
          pango
          gdk-pixbuf
          freetype
          fontconfig
          harfbuzz
          librsvg
          libxml2
          openssl
        ];

        nativeDeps = with pkgs; [
          pkg-config
          clang
          llvm
        ];

        pkgConfigPath = pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" (with pkgs; [
          gtk4.dev
          libadwaita.dev
          glib.dev
          cairo.dev
          pango.dev
          gdk-pixbuf.dev
          freetype.dev
          fontconfig.dev
          harfbuzz.dev
        ]);

        libclangPath = "${pkgs.libclang.lib}/lib";
      in {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "network-monitor";
          version = "0.7.2";
          src = ./.;

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = nativeDeps;

          buildInputs = gtkDeps;

          LIBCLANG_PATH = libclangPath;
          PKG_CONFIG_PATH = pkgConfigPath;

          postInstall = ''
            install -Dm644 network-monitor.desktop -t $out/share/applications
            install -Dm644 icons/hicolor/scalable/apps/network-monitor.svg \
              -t $out/share/icons/hicolor/scalable/apps
          '';

          meta = with pkgs.lib; {
            description = "Network monitoring application with GTK4 and TUI interfaces";
            homepage = "https://github.com/grigio/network-monitor";
            license = licenses.gpl3Plus;
            platforms = platforms.linux;
            mainProgram = "network-monitor";
          };
        };

        devShells.default = pkgs.mkShell {
          name = "network-monitor-shell";

          inputsFrom = [ self.packages.${system}.default ];

          packages = with pkgs; [
            rust-analyzer
            bpf-linker
            cargo-outdated
            cargo-audit
            cargo-deny
            rustup
          ];

          LIBCLANG_PATH = libclangPath;
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath gtkDeps;

          shellHook = ''
            echo ""
            echo "🦀 Network Monitor Dev Shell"
            echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            echo "  Rust:  $(rustc --version)"
            echo "  Cargo: $(cargo --version)"
            echo ""
            echo "  nix build          → build project"
            echo "  nix fmt            → format nix files"
            echo "  cargo build        → (via rustup, with eBPF)"
            echo "  cargo clippy       → lint"
            echo "  cargo fmt --check  → format"
            echo ""
            echo "  Nightly + bpf-linker for eBPF:"
            echo "  rustup toolchain install nightly"
            echo "  rustup target add bpfel-unknown-none --toolchain nightly"
            echo "  cargo install bpf-linker"
            echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
          '';
        };

        formatter = pkgs.nixpkgs-fmt;
      });
}
