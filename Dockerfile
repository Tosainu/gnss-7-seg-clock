FROM ghcr.io/kicad/kicad:10.0.4@sha256:770e2fa006695077b5ba218742d5c3137896af91257b2ca971243fa56b01bb86 AS kicad
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
