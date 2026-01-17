# PNIDGrab
PNIDGrab is a cross-platform PID Grabber for Splatoon on Cemu. It grabs the Principle ID, the Pretendo Network ID and all information about the player's Inkling (or Octo). Everything you'll ever need to report cheaters to the Network Moderators (aside from video evidence of course)!

<img width="1741" height="939" alt="image" src="https://github.com/user-attachments/assets/27073f0a-8450-4b81-b271-b860a86bb225" />

## Important notes
### Windows
Because PNIDGrab goes in Cemu's memory and reads from it, anti virus software may flag it. This is a false positive. The only way to get around it is to add it as an exception. Not much I can do about it, unfortunately.

### macOS
The binary is not signed by Apple, because I don't pay Apple $99 per year for a paid developer account. You will get an error saying that Apple can't verify that this application is free from malware. The easiest solution to that issue is opening a Terminal and running `sudo xattr -cr /Applications/PNIDGrab.app/`. After that, you can run the application as usual and won't have to redo this step again until you update the application.

### Linux
With certain Linux Distros, you may encounter a weird issue with their permission system if you run a Wayland session. Even though you get Polkit to ask for your password, the application will likely not run. [This is because by default ptrace protection is enabled.](https://wiki.ubuntu.com/SecurityTeam/Roadmap/KernelHardening#ptrace_Protection)
>This behavior is controlled via the /proc/sys/kernel/yama/ptrace_scope sysctl value. The default is "1" to block non-child ptrace.

**Extremely dangerous workaround** as of right now to achieve the use of PNIDGrab is to go in `/etc/sysctl.d/`, make a file called `10-ptrace.conf` and put `kernel.yama.ptrace_scope = 0` in there.

Alternatively: you can do `su root` and run the AppImage that way.

**If you wish to not subject yourself to this!!** I'd recommend to try and run it on a different distro inside a Distrobox. If you don't wish to use a Distrobox or that fails too, grab the last CLI-only release (2.x).

## Credits
* [c8ff](https://github.com/c8ff) for finding a method to get Cemu's base address without reading the log file
* [javiig8](https://github.com/javiig8) for finding the addresses to get Name and PID
* [Tombuntu](https://github.com/ReXiSp) for finding the address to get the Session ID
* [CrafterPika](https://github.com/CrafterPika/) for helping with the implementations for macOS and Windows
* [RusticMaple](https://github.com/RusticMaple) for the idea how to split platforms without anything clashing
* [vyrval](https://github.com/tvyrval) for help with getting the Gear and Weapon IDs as well as the pointers for everything other than PID, PNID and Session ID
* [oomi_the_octo](https://github.com/oomi-the-octo) for the pointers for the other Weapon related IDs
