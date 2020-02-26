{ sources ? import ./nix/sources.nix
, pkgs ? import sources.nixpkgs {}
, naersk ? let
    rust = import ./nix/rust.nix { inherit sources; };
  in
    pkgs.callPackage sources.naersk {
      rustc = rust.rust;
      cargo = rust.rust;
    }
}:
let
  src = builtins.filterSource
    (path: type: type != "directory" || builtins.baseNameOf path != "target")
    ./.;
in

naersk.buildPackage {
  src = src;
  buildInputs = with pkgs;
    [
      pkgconfig
      gtk3
      glib
    ];
  doCheck = false;
  postInstall = ''
    mkdir -p $out/share/applications
    cp ${src}/iv.desktop $out/share/applications
  '';
}
