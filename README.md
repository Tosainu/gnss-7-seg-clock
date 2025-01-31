# GNSS Seven-segment Clock

GNSS-powered, seven-segment table clock.

## License

The project is licensed under the [MIT](./LICENSE) license unless otherwise stated.

PCD design files, specifically the files under the [`hardware/`](./hardware/) directory are licensed under the [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) license. Please note that:
- Footprint files described below are made based on the [KiCad Footprint Libraries](https://gitlab.com/kicad/libraries/kicad-footprints) which is licensed under the [CC-BY-SA 4.0 license with exceptions](https://gitlab.com/kicad/libraries/kicad-footprints/-/blob/8.0.8/LICENSE.md?ref_type=tags).
    - [`Display_7Segment_1.5inch_P2.45mm.kicad_mod`](./hardware/gnss-7-seg-clock.pretty/Display_7Segment_1.5inch_P2.45mm.kicad_mod)
    - [`QFN-56-1EP_7x7mm_P0.4mm_EP3.2x3.2mm_HandSolder.kicad_mod`](./hardware/gnss-7-seg-clock.pretty/QFN-56-1EP_7x7mm_P0.4mm_EP3.2x3.2mm_HandSolder.kicad_mod)
    - [`SW_SPST_PTS810_HandSolder.kicad_mod`](./hardware/gnss-7-seg-clock.pretty/SW_SPST_PTS810_HandSolder.kicad_mod)
    - [`USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal_HandSolder.kicad_mod`](./hardware/gnss-7-seg-clock.pretty/USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal_HandSolder.kicad_mod)
- Circuits around the MAX-M10S module on the schematic are made based on [sparkfun/SparkFun\_u-blox\_MAX-M10S][SparkFun-MAX-M10S] which is licensed uner the [CC BY-SA 4.0](https://github.com/sparkfun/SparkFun_u-blox_MAX-M10S/blob/8e937406ba0f21e3afc8ca20ddeb06b088023951/LICENSE.md#hardware) license for the hardeare part.

[SparkFun-MAX-M10S]: https://github.com/sparkfun/SparkFun_u-blox_MAX-M10S
