`frhop {list of directories or nsps}`  
> Note; first time users must setup the [USB driver](#usb-driver). 
---
Tiny utility to serve Switch archives over [Tinfoil](https://tinfoil.io/)'s USB interface - a lightweight (~500kb!) alternative to [`nut.py`](https://github.com/blawar/nut).  

# `frhop` vs `nut`
- Speed-wise it's slightly faster; ~10% faster.  
- Pure rust + completely static - no fiddling with `pip` + all platforms supports  
- Only `nut`'s USB functionality implemented 
- `nut` requires filenames to contain TitleID, `frhop` can extract from `nsp`
- Couple of other QoL improvements that should fix hangs `USB` users may have experienced with `nut`

# Limitations 
Tinfoil's USB interface can be a bit finicky at times, here are the most common ones. Note, everything here affects `nut.py` as well.  
- USB connection may not be restored if you unplug and replug the Switch with Tinfoil opened - restart Tinfoil or put the Switch to sleep and wake again
- Tinfoil only gets NSP listing at the start, to update you must close/re-open the app
- Tinfoil is (tragically) one-threaded - concurrent downloads not possible
- Once you connect USB, Tinfoil will freeze for 1-2 seconds as it parses the NSP headers to extract key info

# USB driver
Depending on your platform, driver setup may be required.  
## Mac
Works without any additional configuration 🥳 
## Windows
Windows users must install the `WinUSB` driver using [`Zadig`](https://zadig.akeo.ie)

**Note**; if you have used [`nut`](https://github.com/blawar/nut) before, `tinfoil_driver.exe` does not install the right driver needed here - follow these instructions to override with the correct driver 

1. Download [`Zadig`](https://zadig.akeo.ie)
2. Open `Tinfoil`, and plug in the switch
3. `Options > List all devices`
4. Select `Tinfoil (Interface 0)` from the dropdown
5. Select the WinUSB (v`...`) driver using the arrow keys, and press install   

![Zadig interface](res/zadig.png)
## Linux
Will need to configure `udev` rules. Follow [`these`](https://docs.rs/nusb/latest/nusb/#linux) instructions. 
# Building
- No special steps, simply install [Rust](https://www.rust-lang.org) and build with Cargo
- To simpliy cross-compilation, I use [zig-build](https://github.com/rust-cross/cargo-zigbuild)

# Tasks
> - ~~Allow user to specify list of directories + files (checkout my other project on github - already done)~~
> - ~~Make multi-threaded~~
> - ~~Move Listing out of TinfoilDevice~~
> - ~~Make QueryError non-fatal (*difficult -> I need to get a way to propagate out Vec<u8> when using '?')~~
> - ~~Dynamically get nsp~~
> - ~~Drop the device correctly \(i.e., all you have to do is override ctrl+c) on ctrl+c else you'll get erros when restarting program~~
> - ~~ALSO, once I figure out a neat way to get Vec\<u8> back to Interface on QueryError, I can appropriately report error to the switch~~
>     - E.g., if user accidently goes onto UsbFS, switch freezes until response is returned -> program doesn't support it, so I should return an empty \[JSON] response
. - Add a file watcher - more difficult than it seems