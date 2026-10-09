import time


def hello():
    message = "Hello from Czar!"
    print(message)
    return {"message": message, "timestamp": time.time()}


def wait():
    time.sleep(1)
    print("waited 1s")
    return {"waited_seconds": 1}


def note():
    note_text = "all done"
    print(note_text)
    return {"note": note_text, "finished_at": time.time()}
