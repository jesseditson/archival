export default async (params, body, objects) => ({
  name: params.get("name"),
  body,
  artist: objects.artist[0].name,
  site: objects.SITE_URL,
});
