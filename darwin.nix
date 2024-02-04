{ nixpkgs ? import <nixpkgs> {} }:

let
  pkgs = [
    nixpkgs.openssl
    nixpkgs.cargo
    nixpkgs.pkg-config
    nixpkgs.darwin.apple_sdk.frameworks.Security
    nixpkgs.darwin.libiconv
    nixpkgs.curl
  ];

in
  nixpkgs.stdenv.mkDerivation {
    name = "env";
    buildInputs = pkgs;
  }
