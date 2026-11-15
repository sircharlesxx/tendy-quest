extends Area3D

@export var lifetime: float = 5.0
@export var slow_duration: float = 0.4

func _ready() -> void:
	body_entered.connect(_on_body_entered)
	var timer := get_tree().create_timer(lifetime)
	timer.timeout.connect(queue_free)

func _physics_process(_delta: float) -> void:
	for body in get_overlapping_bodies():
		if body.is_in_group("player") and body.has_method("apply_fiber_slow"):
			body.apply_fiber_slow(slow_duration)

func _on_body_entered(body: Node3D) -> void:
	if body.is_in_group("player") and body.has_method("apply_fiber_slow"):
		body.apply_fiber_slow(slow_duration)
