{ pkgs, lib, config, inputs, ... }:

{
  packages = with pkgs; [
    libz
    black
  ];
  
  languages.python = {
    enable = true;
    version = "3.14.6";
    lsp.enable = true;
    lsp.package = pkgs.python314Packages.python-lsp-server;
    venv.enable = true;
    poetry.enable = true;
    poetry.install.enable = true;
  };
}
