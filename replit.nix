{ pkgs }: {
  deps = [
    pkgs.curl
    pkgs.openssl
    pkgs.pkg-config
    pkgs.vim
  ];
}