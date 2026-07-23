import { useEffect, useState } from 'react';
import { getMediaFileBlob } from './api';

export function useAuthenticatedFileUrl(path: string | undefined, agentId?: string): string | undefined {
  const [url, setUrl] = useState<string>();

  useEffect(() => {
    if (!path) {
      setUrl(undefined);
      return;
    }
    let active = true;
    let objectUrl: string | undefined;
    void getMediaFileBlob(path, agentId).then((blob) => {
      if (!active) return;
      objectUrl = URL.createObjectURL(blob);
      setUrl(objectUrl);
    }).catch(() => {
      if (active) setUrl(undefined);
    });
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [path, agentId]);

  return url;
}
