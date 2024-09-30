import { isRecordingAtom, recordingsAtom } from "@/lib/fft";
import { getMicrophoneStream } from "@/lib/getMicrophoneStream";
import { useAtom, useAtomValue } from "jotai";
import { MicIcon, MicOffIcon } from "lucide-react";
import { nanoid } from "nanoid";
import { useCallback, useEffect, useRef, useState } from "react";

export function RecordButton() {
  const [isRecording, setIsRecording] = useAtom(isRecordingAtom);

  const recordings = useAtomValue(recordingsAtom);
  const [stream, setStream] = useState<MediaStream | null>(null);

  const idRef = useRef("");
  const recorderRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);

  const audioElementRef = useRef<HTMLAudioElement | null>(null);

  const startRecording = useCallback(() => {
    if (!stream) return;

    if (!idRef.current) {
      idRef.current = nanoid();
    }

    if (audioElementRef.current) {
      audioElementRef.current.id = idRef.current;
    }

    if (!recorderRef.current) {
      recorderRef.current = new MediaRecorder(stream);

      recorderRef.current.ondataavailable = (e) => {
        chunksRef.current.push(e.data);
      };

      recorderRef.current.onstop = () => {
        const oggBlob = new Blob(chunksRef.current, {
          type: "audio/ogg; codecs=opus",
        });
        recordings.set(idRef.current, oggBlob);
      };
    }

    recorderRef.current.start();
  }, [recordings, stream]);

  const stopRecording = useCallback(() => {
    if (!recorderRef.current) return;

    recorderRef.current.stop();
  }, []);

  const toggleRecording = useCallback(() => {
    setIsRecording((prevIsRecording) => {
      if (prevIsRecording) {
        stopRecording();
      } else {
        startRecording();
      }

      return !prevIsRecording;
    });
  }, [setIsRecording, startRecording, stopRecording]);

  useEffect(() => {
    getMicrophoneStream().then(setStream);

    const handleKeypress = (ev: KeyboardEvent) => {
      if (ev.key === " ") {
        toggleRecording();
      }
    };

    window.addEventListener("keydown", handleKeypress);

    return () => {
      window.removeEventListener("keydown", handleKeypress);
    };
  }, [setIsRecording, toggleRecording]);

  return (
    <>
      <button
        className="grid h-full border rounded-md shadow-sm place-items-center aspect-square border-input bg-background hover:bg-accent hover:text-accent-foreground"
        onClick={toggleRecording}
      >
        {isRecording ? (
          <span className="flex flex-col items-center gap-1">
            <MicOffIcon size={16} /> Stop
          </span>
        ) : (
          <span className="flex flex-col items-center gap-1">
            <MicIcon size={16} /> Start
          </span>
        )}
      </button>
      <audio ref={audioElementRef} />
    </>
  );
}
