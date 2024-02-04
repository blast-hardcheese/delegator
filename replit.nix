{ pkgs }: {
  deps = [
    pkgs.curl
    pkgs.openssl
    pkgs.pkg-config
    pkgs.vim
  ];
  env = {
    HISTFILE = "$REPL_HOME/.cache/bash_history";
  };
}