extends CenterContainer

class_name LayoutConfigContainer

signal exit()

@onready var dense_layout_checkbox: CheckBox = %DenseLayoutCheckBox

# Called when the node enters the scene tree for the first time.
func _ready():
	pass # Replace with function body.


func _on_back_button_pressed():
	exit.emit()


func focus():
	dense_layout_checkbox.grab_focus()
