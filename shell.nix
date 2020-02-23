let
  pkgs = import <nixpkgs> {};
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    gtk3
    glib
    pkgconfig
  ];
}
