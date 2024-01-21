{ pkgs }: {
  deps = [
    pkgs.rustup
    pkgs.vim
    pkgs.openssl
    pkgs.pkg-config
  ];
}