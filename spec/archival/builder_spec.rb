# frozen_string_literal: true

require 'archival'

FIXTURES_DIR = File.join(File.dirname(__FILE__),
                         '..', 'fixtures')

def write_snapshot(name, content)
  File.open(
    File.join(FIXTURES_DIR, 'snapshots',
              name), 'w+'
  ) do |file|
    file.write(content)
  end
end

def snapshot(name)
  File.read(File.join(FIXTURES_DIR, 'snapshots',
                      name))
end

RSpec.describe Archival::Builder do
  before(:each) do
    root = File.join(FIXTURES_DIR,
                     'simple_website')
    Layout.reset_cache
    config = Archival::Config.new('root' => root)
    @builder = Archival::Builder.new(config)
    Dir.chdir root
  end
  context 'sort_objects' do
    it 'sort_objects with order' do
      objects = {}
      objects['last'] = { 'bar' => 'baz', 'order' => 5 }
      objects['first'] = { 'buzz' => 'baz', 'order' => 1 }
      sorted = @builder.sort_objects(objects)
      expect(sorted.length).to eq(2)
      expect(sorted[0]).to eq(objects['first'])
      expect(sorted[1]).to eq(objects['last'])
      expect(sorted['first']).to eq(objects['first'])
    end
    it 'sort_objects by name' do
      objects = {}
      objects['2'] = { 'bar' => 'baz' }
      objects['1'] = { 'buzz' => 'baz' }
      sorted = @builder.sort_objects(objects)
      expect(sorted.length).to eq(2)
      expect(sorted[0]).to eq(objects['1'])
      expect(sorted[1]).to eq(objects['2'])
    end
  end
  context 'simple website' do
    it 'has the right pages' do
      expect(@builder.page_templates.keys).to eq ['index']
    end
    it 'renders the index page' do
      out = @builder.render('index')
      if ENV['WRITE_SNAPSHOT']
        write_snapshot('simple_website_index',
                       out)
      end
      expect(out).to eq snapshot('simple_website_index')
    end
  end
end
