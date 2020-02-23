{ sources ? import ./nix/sources.nix }:
let
  pkgs = import sources.nixpkgs {};
  rust = import ./nix/rust.nix { inherit sources; };
  naersk = pkgs.callPackage sources.naersk {
    rustc = rust.rust;
    cargo = rust.rust;
  };
in
naersk.buildPackage {
  src = builtins.filterSource
    (path: type: type != "directory" || builtins.baseNameOf path != "target")
    ./.;
  buildInputs = with pkgs; [
    pkgconfig
    gtk3
    glib
  ];
  doCheck = false;
}
