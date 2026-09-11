/**
 * The archive formats import can open, lowercase and without the leading dot.
 *
 * One list, because three places need to agree about it: the file picker's filter, the drop handler
 * that decides whether a dragged file is a mod, and the image drop handler, which has to recognise an
 * archive in order to stay quiet and let the importer have it.
 */
export const ARCHIVE_EXTENSIONS = ['zip', '7z', 'rar'];

export function isArchivePath(path: string): boolean {
   const extension = path.split('.').pop()?.toLowerCase() ?? '';
   return ARCHIVE_EXTENSIONS.includes(extension);
}
