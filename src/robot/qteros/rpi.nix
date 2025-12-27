{ config, pkgs, ... }:

# Bookmark: https://carjorvaz.com/posts/nixos-on-raspberry-pi-4-with-uefi-and-zfs/
# Also make sure `/` is mounted from the `@root` subvolume.

{
  imports = [
    ./rpi-hardware.nix
    ./software.nix
  ];

  networking.hostName = "qter-robot";

  # Bootloader and kernel stuff

  boot.loader.generic-extlinux-compatible.enable = true;
  boot.loader.efi.canTouchEfiVariables = true;
  # boot.loader.systemd-boot = {
  #   enable = true;
  #   # This is where the RPI gets kernal parameters from for whatever reason
  #   extraFiles."cmdline.txt" = builtins.toFile "cmdline.txt" "${builtins.toString config.boot.kernelParams} init=/nix/var/nix/profiles/system/init";
  # };
  boot.kernelParams = [
    "iomem=relaxed"
    "strict-devmem=0"
  ];

  # Packages

  environment.systemPackages = with pkgs; [
    libraspberrypi
    raspberrypi-eeprom
    wireguard-tools
  ];

  # SSH

  services.openssh.enable = true;

  # https://wiki.nixos.org/wiki/NixOS_on_ARM/Raspberry_Pi_4

  # Create gpio group
  users.groups.gpio = {};

  # Change permissions gpio devices
  services.udev.extraRules = ''
    SUBSYSTEM=="bcm2835-gpiomem", KERNEL=="gpiomem", GROUP="gpio",MODE="0660"
    SUBSYSTEM=="gpio", KERNEL=="gpiochip*", ACTION=="add", RUN+="${pkgs.bash}/bin/bash -c 'chown root:gpio /sys/class/gpio/export /sys/class/gpio/unexport ; chmod 220 /sys/class/gpio/export /sys/class/gpio/unexport'"
    SUBSYSTEM=="gpio", KERNEL=="gpio*", ACTION=="add",RUN+="${pkgs.bash}/bin/bash -c 'chown root:gpio /sys%p/active_low /sys%p/direction /sys%p/edge /sys%p/value ; chmod 660 /sys%p/active_low /sys%p/direction /sys%p/edge /sys%p/value'"
  '';

  age.secrets.pass = {
    file = ./secrets/pass.age;
    owner = "robot";
  };

  users = {
    users.robot = {
      hashedPasswordFile = config.age.secrets.pass.path;
      extraGroups = [ "gpio" ];
      openssh.authorizedKeys.keys = import ./ssh-keys.nix;
    };
  };

  # Allow hosting random stuff

  networking.firewall = {
    enable = true;
    allowedTCPPortRanges = [
      { from = 1000; to = 9000; }
    ];
  };

  # btrfs backups

  environment.etc = {
    "btrbk/btrbk.conf".text = ''
      timestamp_format long
      snapshot_preserve_min 16h
      snapshot_preserve 48h 7d 3w 4m 1y

      volume /
      	snapshot_dir btrbk-snapshots
      	subvolume .
    '';
  };

  system.stateVersion = "23.11";
}
