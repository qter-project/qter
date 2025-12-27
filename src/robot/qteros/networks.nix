{ config, pkgs, ... }:

# MAC changer from here: https://nixos.wiki/wiki/Wpa_supplicant

{
  age.secrets.wireless-environment-file.file = ./secrets/wireless-environment-file.age;
  # `network.wireless.interfaces` is expected to be set elsewhere
  networking.wireless = {
    enable = true;

    userControlled.enable = true;

    secretsFile = config.age.secrets.wireless-environment-file.path;

    networks = {
      "Fuse.SynergyWifi.com".pskRaw = "ext:fuse_wifi_pass";
    };
  };

  # Hotspot
  services.create_ap = {
    enable = true;
    settings = {
        PASSPHRASE = "12345678";
        SSID = "Qter Robot";
        WIFI_IFACE = "wlan0";
    };
  };
}
