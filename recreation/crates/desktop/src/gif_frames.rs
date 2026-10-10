//! Bounded GIF sampling preserves the full accepted loop instead of rejecting it at the old 120-frame ceiling.
use image::{Frame,Delay};
pub struct Frames{pub frames:Vec<Frame>,seen:usize,stride:usize,pub bytes:usize}
impl Default for Frames{fn default()->Self{Self{frames:Vec::new(),seen:0,stride:1,bytes:0}}}
fn milliseconds(frame:&Frame)->u32{std::time::Duration::from(frame.delay()).as_millis().clamp(20,10_000)as u32}
fn merge_delay(frame:&mut Frame,extra:u32){
    let delay=std::time::Duration::from(frame.delay()).as_millis().min(u32::MAX as u128)as u32;
    let old=std::mem::replace(frame,Frame::new(image::RgbaImage::new(0,0)));
    let (x,y)=(old.left(),old.top());let buffer=old.into_buffer();*frame=Frame::from_parts(buffer,x,y,Delay::from_numer_denom_ms(delay.saturating_add(extra),1));
}
impl Frames{
    pub fn push(&mut self,frame:Frame)->Result<(),()>{
        if self.seen>=600{return Err(());}
        let source_index=self.seen;self.seen+=1;
        let delay=milliseconds(&frame);
        if source_index%self.stride!=0{
            if let Some(last)=self.frames.last_mut(){merge_delay(last,delay);}return Ok(());
        }
        if self.frames.len()>=240{
            let mut compact=Vec::with_capacity(120);let mut old=std::mem::take(&mut self.frames).into_iter();
            while let Some(mut first)=old.next(){if let Some(second)=old.next(){let duration=std::time::Duration::from(second.delay()).as_millis()as u32;merge_delay(&mut first,duration);}compact.push(first);}
            self.frames=compact;self.stride*=2;
            self.bytes=self.frames.iter().map(|f|f.buffer().len()).sum();
            if source_index%self.stride!=0{if let Some(last)=self.frames.last_mut(){merge_delay(last,delay);}return Ok(());}
        }
        let buffer=frame.into_buffer();
        let (w,h)=buffer.dimensions();
        if w==0||h==0||w>1024||h>1024{return Err(());}
        let scale=(192.0/w as f64).min(128.0/h as f64).min(1.);
        let (width,height)=((w as f64*scale).round().max(1.)as u32,(h as f64*scale).round().max(1.)as u32);
        let buffer=if (width,height)!=(w,h){image::imageops::resize(&buffer,width,height,image::imageops::FilterType::Triangle)}else{buffer};
        self.bytes+=buffer.len();self.frames.push(Frame::from_parts(buffer,0,0,Delay::from_numer_denom_ms(delay,1)));Ok(())
    }
}
