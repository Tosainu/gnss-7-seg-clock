# GNSS Seven-segment Clock

GNSS-powered, seven-segment table clock.

![DSC_8405](https://github.com/user-attachments/assets/dfdbf40c-fb0f-4bc4-b784-38103c194dda)

https://github.com/user-attachments/assets/9b2d0560-d879-4ddb-b99d-07a6e9839360

Full video: https://www.youtube.com/watch?v=Y1P_iDvq4bk

## Firmware

### Usage

Pre-build firmware files are available on the [release page](https://github.com/Tosainu/gnss-7-seg-clock/releases). Thanks to the RP2040 Bootrom, flashing the firmware requires no special tool. Connect the board to the PC using the USB-C cable, while holding the BOOT button (`SW2`). Once the `RPI-RP2` drive appears on the PC, copy `gnss-7-seg-clock.uf2` there. The board will automatically be rebooted as soon as it finishes writing to Flash.

- The board displays `--.--.--` until it obtains the time information.
- Press `SW3` to change the display contents:
    1. Time: `hh.mm.ss`
    2. Date: `YY.MM.DD`
    3. Configuring time zone (time offset): `[-]hh.mm`
        - `SW4`: + 30 min
        - `SW5`: - 30 min

### Build

The firmware is written in the [Rust][rust] programming language with the [Embassy][embassy] framework. In order to build the firmware, you first need to prepare the Rust toolchain. Please refer to the [official guide][rustup].

Next, install the linker helper [`flip-link`][flip-link]. Alternatively, remove [this line](https://github.com/Tosainu/gnss-7-seg-clock/blob/446c9426d9386b7eddf7218308c7567423a233c5/.cargo/config.toml#L11) to use the default linker.

    $ cargo install --locked flip-link

Lastly, run the command below to build the firmware. You can find the ELF file `./target/thumbv6m-none-eabi/release/gnss-7-seg-clock` afterward.

    $ cargo build --release

If the debug probe is attached to the board and [probe-rs][probe-rs] is installed on your PC, `cargo run` can be used to load firmware to the board.

    $ cargo run --release
    
    # to run test apps in crates/gnss-7-seg-clock/examples/
    $ cargo run --release --example gnss-uart-pipe

## Schematic and PCB

> [!IMPORTANT]
> I'm new to circuits and PCB designs. Any feedback and advice are welcome! (✿ゝ◡╹)ノ

I designed the Rev.A board with KiCad v8.0.8 (Linux/macOS) and their official libraries. You can find the KiCad project files in the [`hardware/`](./hardware/) directory. You can also find the rendered schematic (PDF) and Gerber files on the [release page](https://github.com/Tosainu/gnss-7-seg-clock/releases/tag/hardware%2Frev-a).

![DSC_7465](https://github.com/user-attachments/assets/b8ba4ea2-d15b-40a8-b685-3bc720fe4624)

![DSC_7464](https://github.com/user-attachments/assets/bad113b0-ff11-46c8-9ca2-d24075941334)
<sub>
Please do not take a close look at 43-44 pins of RP2040! (ignorable solder bridge)
</sub>

### Parts

> [!NOTE]
> Tolerances were set by just referring to several existing designs. Some of them might be insufficient or overkill.

| Reference | Value | Parts | Qty |
| --- | --- | --- | --: |
| `C1`, `C2`, `C4-C10`, `C14`, `C17-C19` | MLCC, 0.1uF, 10%, 6.3V, X5R, M1005 | | 13 |
| `C3`, `C11` | MLCC, 1uF, 20%, 6.3V, X5R, M1005 | | 2 |
| [`C12`, `C13`](#ldo-and-capacitors) | MLCC, 10uF, 10%, 6.3V, X7R, M2012 | | 2 |
| `C15`, `C16` | MLCC, 15pF, 5%, 50V, C0G/NP0, M1005 | | 2 |
| [`C20`](#rf-circuitry-for-max-m10s) | MLCC, 10000pF, 10%, 16V, X7R, M1005 | | 1 |
| [`C21`](#rf-circuitry-for-max-m10s) | MLCC, 47pF, 5%, 50V, C0G/NP0, M1005 | | 1 |
| [`D1`](#rf-circuitry-for-max-m10s) | Bidirectional TVS, M1005 | [Littelfuse PESD0402-140][PESD0402-140] | 1 |
| `D2-D7` | LED, M1005 | | 6 |
| `J1` | USB-C Receptacle | [GCT USB4105-GF-A](https://gct.co/connector/usb4105) | 1 |
| `J2` | SMA Receptacle, Edge Mount | [Molex 732511153][732511153] | 1 |
| `J3` | Pin header, 01x03, P2.54 mm | | 1 |
| [`L1`](#rf-circuitry-for-max-m10s) | Inductor, 27nH, 5%, M1005 | [Murata LQG15HS27NJ02D][LQG15HS27NJ02D] | 1 |
| [`R1-R3`](#driving-seven-segment-leds-with-fewer-pins) | Resistor, 2.2kOhm, 1%, 1/16W, M1005 | | 3 |
| `R4`, `R5`, `R12`, `R14-R16` | Resistor, 10kOhm, 1%, 1/16W, M1005 | | 6 |
| `R6`, `R7` | Resistor, 5.1kOhm, 1%, 1/16W, M1005 | | 2 |
| `R8`, `R9` | Resistor, 27Ohm, 1%, 1/16W, M1005 | | 2 |
| `R10`, `R13` | Resistor, 1kOhm, 1%, 1/16W, M1005 | | 2 |
| [`R11`](#rf-circuitry-for-max-m10s) | Resistor, 10Ohm, 5%, 1/4W, M1005 | | 1 |
| `R17-R22` | Resistor, 470Ohm, 1%, 1/16W, M1005 | | 6 |
| `SW1-SW5` | Tactile Switch | [C&K PTS810][PTS810] | 5 |
| [`U1-U6`](#seven-segment-leds) | 7-segment LED, Common-Anode, 3.81 mm | | 6 |
| [`U7-U9`](#driving-seven-segment-leds-with-fewer-pins) | 16-ch LED sink driver, SOIC-24W | [TI TLC5925IDWR][TLC5925] | 3 |
| `U10` | MCU | [Raspberry Pi RP2040][RP2040] | 1 |
| [`U11`](#ldo-and-capacitors) | LDO, 3.3V, 500 mA, SOT-23-5 | [TI TLV75533PDBVR][TLV755P] | 1 |
| `U12` | SQPI NOR Flash, 32M-bit, SOIC-8 | [Winbond W25Q32JVSS][W25Q32JV] | 1 |
| `U13` | GNSS Receiver | [u-blox MAX-M10S][MAX-M10] | 1 |
| `Y1` | Crystal, 12MHz | [Abracon ABM8-272-T3][ABM8-272-T3] | 1 |
| | GNSS Active Antenna | [u-blox ANN-MB5][ANN-MB5] | 1 |

### Seven-segment LEDs

`U1-U6` are seven-segment LEDs, the essential components in this design. While selecting them, I was surprised about the poor availability of seven-segment LEDs, especially the larger models (> 1″). I wanted to use [Kingbright SA15-11GWA][SA15-11GWA] initially. However, due to the availability and price, I selected [WENRUN LSD150BAG-101][LSD150BAG-101], even though I had to use a different store from other parts.

Unfortunately, LSD150BAG-101 is not a drop-in replacement for SA15-11GWA. They have a different pitch for the vertical direction: 40.64mm (SA15-11GWA) and 40.00mm (LSD150BAG-101). Since I wanted to have some flexibility in the design, I made the footprint that uses the oval pad so that can use both types of seven-segment LEDs.

![Screenshot_2025-02-01-122517](https://github.com/user-attachments/assets/0b0ae403-fe8b-4c34-ab8e-8e7f0641486e)

The forward voltage also has to be taken into account for `U1-U6`. Vf should be smaller than the USB VBUS. For instance, high-luminance types are not suitable in general.

### Driving Seven-segment LEDs with Fewer Pins

`U7`, `U8`, and `U9` are 16-ch shift registers specialized for the LED, [TI TLC5925IDWR](TLC5925). Thanks to this three-cascading configuration, it can drive six seven-segment LEDs (48x LED segments) only by five pins.

TLC5925 determines the output currents based on an external resistor between the `R-EXT` pin and `GND`. `R1`, `R2`, and `R3` are the current-set resistors for `U7`, `U8`, and `U9` respectively. With the 2.2kOhm resistor, the output would be (1.21 / 2,200) \* 18  = 9.9mA.

### LDO and Capacitors

`U11` is the 3.3V LDO, [TI TLV75533PDBVR](TLV755P) (TLV755P-series). I'm hoping it works nicely with an AC adapter/wall charger as well as a PC because of its good PSRR in wide-range frequencies.

`C12` and `C13` are the input and output capacitors for the LDO. According to the datasheet, TLV755P requires a 1uF+ input capacitor and a 0.47uF+ output capacitor while considering the DC bias characteristics of a capacitor. Based on this, I selected 10uF/6.3V MLCC.

### RF Circuitry for MAX-M10S

[sparkfun/SparkFun\_u-blox\_MAX-M10S][SparkFun-MAX-M10S] is very good material as well as the MAX-M10S Integration manual. For the RF part, I imitated SparkFun's board and filled in parts values based on the reference design mentioned in the integration manual. Here are the parts correspondences between this board and the MAX-M10S reference design:

| This board | Reference design | Use |
| --- | --- | --- |
| `C20` | `C14` |  RF Bias-T Capacitor |
| `C21` | `C18` | DC Block Capacitor[^dc-block-cap] |
| `L1` | `L3` | RF Bias-T Inductor |
| `R11` | `R8` | Antenna supervisor current limiter/shunt resistor |

`D1` is the ESD-protection TVS which is only in the SparkFun's board. Since I have no confidence in finding compatible parts, I used the same TVS [Littelfuse PESD0402-140][PESD0402-140] in this design.

[^dc-block-cap]: Apparently, this capacitor can be removed as my design doesn't have an external SAW filter. The built-in DC-block capacitor of the module is sufficient.

## Special Thanks - PCBWay

I would like to say a special thank you to [PCBWay](https://www.pcbway.com/) for reaching out and sponsoring my project. As part of the sponsorship, I had the opportunity to try their PCBA service. Overall I am very satisfied with the service and quality of the PCBs. Especially,

- Swift and kind support
    - I was surprised that they reviewed my design files very quickly and gave me a reply after about an hour from the order.
- Numerous customization options for PCB manufacturing
- Help Center
    - There are how-to specialized for KiCad such as [_"How to Generate Gerber and Drill Files in KiCad 8.0?"_](https://www.pcbway.com/helpcenter/generate_gerber/Generate_Gerber_file_from_Kicad.html) and [_"Generate Position File Centroid File(pick place) in Kicad"_](https://www.pcbway.com/helpcenter/design_instruction/Generate_Position_File_in_Kicad.html). They were very helpful when I generate design files for the order.
- Quality of PCBs
    - The board has RP2040 which is the 0.20 mm pitch IC but there were no major issues. One thing to note is the soldermask color: green would be the only choice for such boards according to their explanations and the document [_"Soldermask Issues - Soldermask bridge"_](https://www.pcbway.com/helpcenter/soldermask_issues/Soldermask_bridge.html).

![250316-133125-DSC_8347](https://github.com/user-attachments/assets/12140d7a-a3ff-4dff-ac41-3c42d2702b02)

Note that I made some changes from the Rev.A design for this order, such as changing footprints to a non-HandSolder variant. If you're interested in about the design files I used to order, please refer to the [Rev.A2 release](https://github.com/Tosainu/gnss-7-seg-clock/releases/tag/hardware%2Frev-a2).

## License

The project is licensed under the [MIT](./LICENSE) license unless otherwise stated.

PCD design files, specifically the files under the [`hardware/`](./hardware/) directory are licensed under the [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) license. Please note that:
- Footprint files described below are made based on the [KiCad Footprint Libraries](https://gitlab.com/kicad/libraries/kicad-footprints) which is licensed under the [CC-BY-SA 4.0 license with exceptions](https://gitlab.com/kicad/libraries/kicad-footprints/-/blob/8.0.8/LICENSE.md?ref_type=tags).
    - [`Display_7Segment_1.5inch_P2.45mm.kicad_mod`](./hardware/gnss-7-seg-clock.pretty/Display_7Segment_1.5inch_P2.45mm.kicad_mod)
    - [`QFN-56-1EP_7x7mm_P0.4mm_EP3.2x3.2mm_HandSolder.kicad_mod`](./hardware/gnss-7-seg-clock.pretty/QFN-56-1EP_7x7mm_P0.4mm_EP3.2x3.2mm_HandSolder.kicad_mod)
    - [`SW_SPST_PTS810_HandSolder.kicad_mod`](./hardware/gnss-7-seg-clock.pretty/SW_SPST_PTS810_HandSolder.kicad_mod)
    - [`USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal_HandSolder.kicad_mod`](./hardware/gnss-7-seg-clock.pretty/USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal_HandSolder.kicad_mod)
- 3D model of ublox MAX module [`MAX (STEP-AP214).STEP`](./hardware/3d_models/MAX%20(STEP-AP214).STEP) is copied from [u-blox/3D-Step-Models-Library@92ce6ba0](https://github.com/u-blox/3D-Step-Models-Library/tree/92ce6ba04a1dacdbec4c5b9e0c032d87bb4d9fc0).

    > Copyright (C) u-blox
    >
    > u-blox reserves all rights in this deliverable (documentation, software, etc., hereafter “Deliverable”).
    >
    > u-blox grants you the right to use, copy, modify and distribute the Deliverable provided hereunder for any purpose without fee.
    >
    > THIS DELIVERABLE IS BEING PROVIDED "AS IS", WITHOUT ANY EXPRESS OR IMPLIED WARRANTY. IN PARTICULAR, NEITHER THE AUTHOR NOR U-BLOX MAKES ANY REPRESENTATION OR WARRANTY OF ANY KIND CONCERNING THE MERCHANTABILITY OF THIS DELIVERABLE OR ITS FITNESS FOR ANY PARTICULAR PURPOSE.
    >
    > In case you provide us a feedback or make a contribution in the form of a further development of the Deliverable (“Contribution”), u-blox will have the same rights as granted to you, namely to use, copy, modify and distribute the Contribution provided to us for any purpose without fee.

- Circuits around the MAX-M10S module on the schematic are made based on [sparkfun/SparkFun\_u-blox\_MAX-M10S][SparkFun-MAX-M10S] which is licensed under the [CC BY-SA 4.0](https://github.com/sparkfun/SparkFun_u-blox_MAX-M10S/blob/8e937406ba0f21e3afc8ca20ddeb06b088023951/LICENSE.md#hardware) license for the hardware part.

This project is inspired by [Kello version 4](http://kair.us/projects/clock/v4/index.html).

[rust]: https://www.rust-lang.org/
[rustup]: https://www.rust-lang.org/tools/install
[embassy]: https://embassy.dev/
[flip-link]: https://github.com/knurling-rs/flip-link
[probe-rs]: https://probe.rs/

[RP2040]: https://www.raspberrypi.com/products/rp2040/
[LQG15HS27NJ02D]: https://www.murata.com/en-global/products/productdetail?partno=LQG15HS27NJ02%23
[LSD150BAG-101]: https://www.tme.eu/en/details/lsd150bag-101/7-segment-led-displays/wenrun/lsd150bag-101-01/
[PESD0402-140]: https://www.littelfuse.com/products/overvoltage-protection/polymer-esd-suppressors/pesd-protection-device/pesd/pesd0402-140
[PTS810]: https://www.ckswitches.com/products/switches/product-details/Tactile/PTS810/
[TLC5925]: https://www.ti.com/product/TLC5925
[TLV755P]: https://www.ti.com/product/TLV755P
[W25Q32JV]: https://www.winbond.com/hq/product/code-storage-flash-memory/serial-nor-flash/?__locale=en&partNo=W25Q32JV
[732511153]: https://www.molex.com/en-us/products/part-detail/732511153
[USB4105]: https://gct.co/connector/usb4105
[ABM8-272-T3]: https://abracon.com/datasheets/ABM8-272-T3.pdf
[ANN-MB5]: https://www.u-blox.com/en/product/ann-mb5-antenna
[MAX-M10]: https://www.u-blox.com/en/product/max-m10-series
[SA15-11GWA]: https://www.kingbrightusa.com/product.asp?catalog_name=LED&product_id=SA15-11GWA

[SparkFun-MAX-M10S]: https://github.com/sparkfun/SparkFun_u-blox_MAX-M10S
