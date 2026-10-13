// www.snyvi.com: one address for the site. Every request goes to the same
// path on snyvi.com, permanently. Only www traffic ever runs this.
export default {
  fetch(request) {
    const url = new URL(request.url);
    url.hostname = "snyvi.com";
    url.protocol = "https:";
    return Response.redirect(url.toString(), 301);
  },
};
