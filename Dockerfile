FROM ghcr.io/kicad/kicad:9.0.7@sha256:4ddaa54d9ead1f1b453e10a8420e0fcfba693e2143ee14b8b9c3b3c63b2a320f AS kicad
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
