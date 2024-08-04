extends Control

class_name PauseScreen

## Note that because we have a process mode of "Always", this
## will always get called, even when the game is paused.
func _unhandled_input(event: InputEvent):
	UserInterface._unhandled_input_always(event)
