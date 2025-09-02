FROM ghcr.io/kicad/kicad:9.0.4@sha256:b8080a1783733c521719dfd443976debdb69b260b7bbc1fc0c7a481b15f3bb22 AS kicad
WORKDIR /work


FROM kicad AS run-erc
COPY --link hardware .
RUN kicad-cli sch erc \
  --format json \
  --output erc.json \
  gnss-7-seg-clock.kicad_sch


FROM scratch AS erc
COPY --from=run-erc /work/erc.json /


FROM kicad AS run-drc
COPY --link hardware .
RUN kicad-cli pcb drc \
  --format json \
  --output drc.json \
  gnss-7-seg-clock.kicad_pcb


FROM scratch AS drc
COPY --from=run-drc /work/drc.json /


FROM kicad AS run-pdf
COPY --link hardware .
RUN kicad-cli sch export pdf \
  --output schematic.pdf \
  gnss-7-seg-clock.kicad_sch

FROM scratch AS pdf
COPY --from=run-pdf /work/schematic.pdf /
