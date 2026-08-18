export default async (params, body, objects) => {
  const upload = await objects.UPLOADS.get(objects.settings.menu);
  return {
    list: await objects.UPLOADS.list(),
    byValue: upload?.key ?? null,
    contents: upload ? await upload.text() : null,
    missing: await objects.UPLOADS.get("not-there.pdf"),
  };
};
