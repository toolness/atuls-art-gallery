extends CenterContainer

class_name LayoutConfigContainer

signal exit()

signal new_layout_complete()

@onready var dense_layout_checkbox: CheckBox = %DenseLayoutCheckBox
@onready var do_layout_button: Button = %DoLayoutButton

# Called when the node enters the scene tree for the first time.
func _ready():
	pass # Replace with function body.


func _on_back_button_pressed():
	exit.emit()


func focus():
	dense_layout_checkbox.grab_focus()


func _on_do_layout_button_pressed():
	var use_dense_layout := dense_layout_checkbox.button_pressed
	get_tree().paused = false
	await MetObjects.layout(use_dense_layout)
	new_layout_complete.emit()
