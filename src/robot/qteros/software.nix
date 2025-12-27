{
  config,
  pkgs,
  lib,
  qter,
  ...
}:

{
  # tmpfs

  fileSystems = {
    "/tmp" = {
      fsType = "tmpfs";
    };
  };

  # Programs

  environment.systemPackages = with pkgs; [
    zoxide
    difftastic
    btrbk
    openssl
    usbutils
    bubblewrap
    nixd
    nixfmt-rfc-style
    htop
    config.boot.kernelPackages.perf
    nnn
    (builtins.getAttr pkgs.system inputs.agenix.packages).default
  ] ++ qter.robot-deps;

  # RTOS stuff
  # boot.kernelPackages = pkgs.linuxPackagesFor (
  #   pkgs.linux_6_12.override {
  #     structuredExtraConfig = with lib.kernel; {
  #       EXPERT = yes;
  #       PREEMPT_RT = yes;
  #       PREEMPT_VARIABLE = no;
  #       RT_GROUP_SCHED = no;
  #     };
  #     ignoreConfigErrors = true;
  #   }
  # );

  # This is still pretty good for low-latency
  boot.kernelParams = [ "threadirqs" "preempt=full" ];

  # Flakes

  nix.settings.experimental-features = [
    "nix-command"
    "flakes"
  ];

  # Main user

  users = {
    mutableUsers = false;
    users.robot = {
      isNormalUser = true;
      extraGroups = [
        "wheel"
        "video"
      ];
      home = "/home/robot";
    };
  };

  # GCing

  nix.gc = {
    automatic = true;
    options = "--delete-older-than 7d";
    dates = "weekly";
  };
  
  # Agenix

  age.identityPaths = [
    "/home/robot/.ssh/id_ed25519"
  ];

  # Enable sudo-rs.

  security.sudo.enable = false;
  security.sudo-rs.enable = true;

  # Polkit

  security.polkit.enable = true;

  # Programs

  home-manager.users.henry = {
    home.stateVersion = "23.11";

    programs.git = {
      enable = true;
      difftastic.enable = true;
      lfs.enable = true;
    };

    programs.helix = {
      enable = true;
      defaultEditor = true;
      languages = builtins.fromTOML (builtins.readFile ./program-configs/helix/languages.toml);
      settings = builtins.fromTOML (builtins.readFile ./program-configs/helix/config.toml);
    };

    programs.direnv = {
      enable = true;
      enableNushellIntegration = true;
      enableBashIntegration = true;
      nix-direnv.enable = true;
    };

    programs.starship = import ./program-configs/starship.nix;
    nix.gc = {
      automatic = lib.mkDefault config.nix.gc.automatic;
      frequency = lib.mkDefault config.nix.gc.dates;
      options = lib.mkDefault config.nix.gc.options;
    };
  };

  users.users.robot.shell = pkgs.zsh;

  environment.shells = [
    pkgs.zsh
  ];

  home-manager.users.root = {
    home.stateVersion = "23.11";

    programs.helix = {
      enable = true;
      defaultEditor = true;
      languages = builtins.fromTOML (builtins.readFile ./program-configs/helix/languages.toml);
      settings = builtins.fromTOML (builtins.readFile ./program-configs/helix/config.toml);
    };

    programs.starship = builtins.fromTOML ./program-configs/starship.toml;
  };

  users.users.root.shell = pkgs.zsh;

  services.zerotierone = {
    enable = true;
    joinNetworks = [
      "bb720a5aaeebb7cb"
    ];
  };

  # btrfs backups; the device specific config file must mount the btrfs partition at /mnt/btrfs

  systemd.timers."btrbk-snapshot" = {
    wantedBy = [ "timers.target" ];
    timerConfig = {
      OnBootSec = "0m";
      OnUnitActiveSec = "1h";
      Unit = "btrbk-snapshot.service";
    };
  };

  systemd.services."btrbk-snapshot" = {
    script = ''
      exec /run/current-system/sw/bin/btrbk -q run
    '';
    serviceConfig = {
      Type = "oneshot";
      User = "root";
    };
  };

  systemd.services.btrbk-snapshots = {
    enable = true;
    description = "Make the btrbk-snapshots directory if it doesn't exist";
    wantedBy = [ "multi-user.target" ];
    script = ''
      if [ ! -d /btrbk-snapshots ]; then
        mkdir /btrbk-snapshots
      fi
    '';
  };

  # SSH

  services.openssh = {
    settings.PasswordAuthentication = false;
  };

  services.avahi.enable = false;
}
