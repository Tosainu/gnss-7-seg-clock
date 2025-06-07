target "_base" {
  dockerfile = "Dockerfile"
  output = ["./out"]
}

target "erc" {
  inherits = ["_base"]
  target = "erc"
}

target "drc" {
  inherits = ["_base"]
  target = "drc"
}

target "pdf" {
  inherits = ["_base"]
  target = "pdf"
}
