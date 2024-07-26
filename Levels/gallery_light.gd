extends Node3D

@onready var light: OmniLight3D = %OmniLight3D


# Called when the node enters the scene tree for the first time.
func _ready():
	_on_potato_mode_changed()
	UserInterface.potato_mode_changed.connect(_on_potato_mode_changed)


func _on_potato_mode_changed():
	light.shadow_enabled = not UserInterface.potato_mode
