target "default" {
  contexts = {
    kicad = "docker-image://kicad/kicad:9.0@sha256:f36b185970d398b84c095ace896046798ff60d458ce59d49669c4633c304cfbd"
  }
  dockerfile-inline = <<EOS
FROM kicad
RUN --mount=type=bind,source=./hardware,target=/work \
  cd /work && kicad-cli sch erc \
    --exit-code-violations \
    --format json \
    --output ~/erc.json \
    gnss-7-seg-clock.kicad_sch; cat ~/erc.json
EOS
  output = [{ type = "cacheonly" }]
}
