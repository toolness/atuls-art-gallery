@tool
extends TextureRect
class_name Reticle


var debug_rect: Rect2:
	set(value):
		debug_rect = value
		queue_redraw()


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
	if debug_rect:
		var abs_rect := Rect2(debug_rect)
		abs_rect.position.x -= get_viewport_rect().size.x / 2
		abs_rect.position.y -= get_viewport_rect().size.y / 2
		draw_rect(abs_rect, Color.GREEN, false)
