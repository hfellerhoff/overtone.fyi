export async function getMicrophoneStream() {
  if (navigator.mediaDevices && navigator.mediaDevices.getUserMedia) {
    try {
      return await navigator.mediaDevices.getUserMedia(
        // constraints - only audio needed for this app
        {
          audio: true,
        }
      );
    } catch (e) {
      console.error(e);
      return null;
    }
  } else {
    console.warn("getUserMedia is not supported in this browser.");
    return null;
  }
}
