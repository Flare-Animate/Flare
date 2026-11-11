//! The small XFL document the XFL import tests share: a main timeline and one library symbol.

pub const DOT: &str = r##"<DOMSymbolItem name="Dot" symbolType="graphic"><timeline><DOMTimeline name="Dot"><layers>
<DOMLayer name="L"><frames><DOMFrame index="0"><elements><DOMShape>
  <fills><FillStyle index="1"><SolidColor color="#FF0000"/></FillStyle></fills>
  <edges><Edge fillStyle1="1" edges="!0 0|200 0|200 200|0 200|0 0"/></edges>
</DOMShape></elements></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timeline></DOMSymbolItem>"##;

pub const MAIN: &str = r##"<DOMDocument frameRate="12" width="400" height="300" backgroundColor="#102030">
<symbols><Include href="Dot.xml"/></symbols>
<timelines><DOMTimeline name="Scene 1"><layers>
  <DOMLayer name="Main" visible="true" locked="true"><frames>
    <DOMFrame index="0" duration="5" tweenType="motion"><elements>
      <DOMSymbolInstance libraryItemName="Dot"><matrix><Matrix tx="10" ty="20"/></matrix></DOMSymbolInstance>
    </elements></DOMFrame>
    <DOMFrame index="5" duration="5"><elements>
      <DOMSymbolInstance libraryItemName="Dot"><matrix><Matrix tx="110" ty="20"/></matrix></DOMSymbolInstance>
    </elements></DOMFrame>
  </frames></DOMLayer>
  <DOMLayer name="Art"><frames><DOMFrame index="0" duration="10"><elements>
    <DOMShape>
      <fills><FillStyle index="1"><SolidColor color="#0000FF" alpha="0.5"/></FillStyle></fills>
      <strokes><StrokeStyle index="1"><SolidStroke weight="2"><fill><SolidColor color="#000000"/></fill></SolidStroke></StrokeStyle></strokes>
      <edges><Edge fillStyle1="1" strokeStyle="1" edges="!4000 2000|4800 2000|4800 2400|4000 2400|4000 2000"/></edges>
    </DOMShape>
    <DOMShape>
      <fills><FillStyle index="1"><SolidColor color="#FFFF00"/></FillStyle></fills>
      <edges><Edge fillStyle1="1" edges="!6000 5000|6200 5000|6200 5200|6000 5200|6000 5000"/></edges>
    </DOMShape>
    <DOMStaticText/>
  </elements></DOMFrame></frames></DOMLayer>
  <DOMLayer name="Cut" layerType="mask"><frames/></DOMLayer>
  <DOMLayer name="Folder" layerType="folder"><frames/></DOMLayer>
</layers></DOMTimeline></timelines></DOMDocument>"##;
