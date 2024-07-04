@tool
extends TextureRect
class_name Reticle

var is_highlighted := false:
	set(value):
		if value != is_highlighted:
			is_highlighted = value
			queue_redraw()

# Draw a circular reticle in the center of the screen.
func _draw() -> void:
	var outer_color := Color.DIM_GRAY
	var inner_color := Color.WHITE_SMOKE
	if is_highlighted:
		outer_color = Color.DARK_GREEN
		inner_color = Color.GREEN
	draw_circle(Vector2.ZERO, 3, outer_color)
	draw_circle(Vector2.ZERO, 2, inner_color)
