interface Crawler {}

class BasicCrawler implements Crawler {}

class HttpCrawler extends BasicCrawler implements Crawler {}

class BrowserCrawler extends BasicCrawler implements Crawler {}

class CollectorBuilder {
  #crawler: Crawler | null = null;

  build(): Collector {
    return new Collector();
  }
}

class Collector {
  #crawler: Crawler | null = null;

  static builder(): CollectorBuilder {
    return new CollectorBuilder();
  }
}

const builder = Collector.builder();
const collector = builder.build();

// const spire = new Spire()
