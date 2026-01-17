#import "../../book.typ": book-page

#show: book-page.with(title: "QterOS and Software Setup")

= QterOS installation

1. Pick an SD card to use
2. Flash the following ISO to the SD card: https://hydra.nixos.org/job/nixos/trunk-combined/nixos.sd_image.aarch64-linux
3. Connect the pi to a USB keyboard, monitor, and ethernet
4. Boot the SD card on the pi
5. Make a `/home/robot` directory and `cd` into it
6. Run `nix-shell -p git` to install git temporarily
6. Clone the qter repository to the pi
7. Install the QterOS config with the following commands

```bash
cd qter/src/robot/qteros
sudo nixos-rebuild boot --flake .#rpi
reboot
```

8. Let the new device into the zerotier network
9. Done question mark? Use these resources for debugging https://rovnyak.net/posts/installing-nix-on-rpi/#Resources

= Installing robot software

TODO

= Running the robot software

TODO
