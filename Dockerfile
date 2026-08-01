FROM ghcr.io/kicad/kicad:10.0.5@sha256:182c8005cb775a2c448a4c18681d489f1ff472a761885eba3e08b07e3c0564de AS kicad
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
