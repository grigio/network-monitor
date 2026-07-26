{
  description = "Network Monitor - Rust + GTK4 + optional eBPF";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let pkgs = nixpkgs.legacyPackages.${system};
      in {
        devShells.default = pkgs.mkShell {
          name = "network-monitor-shell";

          nativeBuildInputs = with pkgs; [
            pkg-config
            gtk4
            libadwaita
            glib
            cairo
            pango
            gdk-pixbuf
          ];

          buildInputs = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            rustup

            bpf-linker

            cargo-outdated
            cargo-audit
            cargo-deny

            freetype
            fontconfig
            harfbuzz
            librsvg
            libxml2
            openssl
            llvm
            libclang
          ];

          shellHook = ''
            echo ""
            echo "🦀 Network Monitor Dev Shell"
            echo "━━━━━━━━━━━━━━━━━━━━━━━━━━"
            echo "  Rust:  $(rustc --version)"
            echo "  Cargo: $(cargo --version)"
            echo ""
            echo "  cargo build                 → eBPF backend"
            echo "  cargo clippy                → lint"
            echo "  cargo fmt --check           → format"
            echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
          '';

          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          PKG_CONFIG_PATH = pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" (with pkgs; [
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
        };
      });
}
