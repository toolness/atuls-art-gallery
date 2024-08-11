extends Window

class_name TeleportDialog

@onready var gallery_id_field: LineEdit = %GalleryIdField

func _unhandled_input(event: InputEvent):
	if event.is_action_pressed("ui_cancel"):
		close_requested.emit()
	if event.is_action_pressed("teleport_dialog"):
		close_requested.emit()

func show_and_focus_ui():
	show()
	gallery_id_field.grab_focus()
