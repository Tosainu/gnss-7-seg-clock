FROM ghcr.io/kicad/kicad:10.0.2@sha256:165c81785b2df23a09892f4cc53bc0095a83b469bfbbe07989670d64049677a7 AS kicad
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
