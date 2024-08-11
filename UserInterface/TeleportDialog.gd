extends Control

class_name TeleportDialog

@onready var gallery_id_field: LineEdit = %GalleryIdField

# Called when the node enters the scene tree for the first time.
func _ready():
	gallery_id_field.grab_focus()

func _unhandled_input(event):
	if event.is_action_pressed("ui_cancel"):
		print("UNHANDLED ", event)
