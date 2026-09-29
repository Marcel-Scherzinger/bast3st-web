import requests
import zipfile
import json


def main(program_path):
    with zipfile.ZipFile(program_path) as f:
        program = f.read("project.json")

    program = json.loads(program)

    resp = requests.post(
        "http://localhost:42139/api/v2/run",
        headers={"Content-Type": "application/json"},
        json={
            "program": program,
            "agent": "python-test",
            "user": "debug",
            "slot": "dyn",
        },
        verify=False,  # as https is self-signed
    )
    print(resp)
    print(resp.text)


if __name__ == "__main__":
    main("file.sb3")
