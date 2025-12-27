let 
  ssh-keys = import ../ssh-keys.nix;
in {
  "pass.age".publicKeys = ssh-keys;
  "wireless-environment-file.age".publicKeys = ssh-keys;
}
