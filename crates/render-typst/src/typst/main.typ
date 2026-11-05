// The render's entry: the screen, its profile's tokens and its images come in as
// `sys.inputs`.
#import "afterimage.typ": screen
#let inputs = sys.inputs
#screen(json(bytes(inputs.screen)), json(bytes(inputs.tokens)), inputs.images, inputs.levels, inputs.width, inputs.height)
