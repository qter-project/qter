#import "../../book.typ": book-page

#show: book-page.with(title: "QterOS and Software Setup")

NOTE: Do this extremely far in advance

= QterOS installation

1. Pick an SD card to use
2. Flash the following ISO to the SD card: https://hydra.nixos.org/job/nixos/trunk-combined/nixos.sd_image.aarch64-linux
3. Mount the second partition
4. In it, create a `/home/robot` directory
5. Clone the qter repository into that directory
6. Connect the pi to a USB keyboard, monitor, and ethernet
7. Boot the SD card on the pi
8. `cd` into `/home/robot/qter/src/robot/qteros`
9. Generate the hardware configuration file with the following commands

```bash
nixos-generate-config --dir .
rm configuration.nix
```

10. Install the QterOS config with the following commands

```bash
# Expect this to take overnight; maybe longer
nixos-rebuild boot --flake .#rpi
reboot
```

11. Let the new device into the zerotier network
12. Done question mark? Use these resources for debugging https://rovnyak.net/posts/installing-nix-on-rpi/#Resources
13. Back up the ISO if you care to

= Installing robot software

TODO

= Running the robot software

TODO
