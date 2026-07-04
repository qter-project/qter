#import "@preview/cetz:0.4.2"
#import "/cube.typ": cube

#cetz.canvas({
  import cetz.draw: *
  cube("wwwwwwwww ggggggggg rrrrrrrrr", name: "cube")
  line((rel: (2, 2), to: "cube.center"), "cube.center", mark: (end: "straight"))
})
